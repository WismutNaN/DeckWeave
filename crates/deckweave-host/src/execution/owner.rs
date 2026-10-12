// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered live owner that connects runtime events to the pure scheduler.

mod activation;
use super::trace::TraceBuffer;
use super::{ExecutionClock, MonotonicExecutionClock};
use super::{
    RuntimeCommand, RuntimeDispatchFailure, RuntimeEvent, RuntimeEventKind, RuntimeEventSink,
    RuntimeIntentDispatcher,
};
use deckweave_application::{RuntimeDiagnosticsSource, RuntimeTraceSnapshot};
mod timers;
mod translate;
use deckweave_automation::{
    BoundExecutionGraph, ExecutableGraph, GraphLimits, IngressContext, IngressItem, IngressKind,
    SchedulerState, WorkspaceScheduler,
};
use deckweave_domain::{
    automation::{CommandOutcome, DeckWeaveError, ErrorCategory, InvocationReply},
    endpoint::RuntimeEndpointRef,
    ids::{ActivationEpoch, GraphNodeId, HostBootId, IngressOrdinal, InvocationId},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use translate::{translate_cleanup, translate_intent};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LiveExecutionError {
    #[error("live automation state lock is poisoned")]
    Poisoned,
    #[error("activation epoch is exhausted")]
    EpochExhausted,
    #[error("ingress ordinal is exhausted")]
    OrdinalExhausted,
    #[error("compiled plan has no executable graph")]
    MissingGraph,
    #[error("runtime binding is incomplete for graph node {0}")]
    MissingBinding(GraphNodeId),
    #[error("live scheduler rejected an event: {0}")]
    Scheduler(String),
}

pub(super) struct ActiveExecution {
    epoch: ActivationEpoch,
    graph: ExecutableGraph,
    scheduler: WorkspaceScheduler,
    bindings: BTreeMap<GraphNodeId, RuntimeEndpointRef>,
    invocation_routes: BTreeMap<InvocationId, RuntimeEndpointRef>,
    next_ordinal: u64,
}

#[derive(Default)]
struct OwnerState {
    next_epoch: u64,
    active: Option<ActiveExecution>,
}

/// One instance owns all live ingress ordering for a workspace. Runtime
/// processes only publish normalized events and execute returned commands.
pub struct LiveExecutionOwner {
    transition_order: Mutex<()>,
    clock: Arc<dyn ExecutionClock>,
    host_boot_id: HostBootId,
    dispatcher: Arc<dyn RuntimeIntentDispatcher>,
    state: Mutex<OwnerState>,
    trace: Mutex<TraceBuffer>,
}

impl LiveExecutionOwner {
    #[must_use]
    pub fn new(host_boot_id: HostBootId, dispatcher: Arc<dyn RuntimeIntentDispatcher>) -> Self {
        Self::with_clock(
            host_boot_id,
            dispatcher,
            Arc::new(MonotonicExecutionClock(std::time::Instant::now())),
        )
    }

    pub fn with_clock(
        host_boot_id: HostBootId,
        dispatcher: Arc<dyn RuntimeIntentDispatcher>,
        clock: Arc<dyn ExecutionClock>,
    ) -> Self {
        Self {
            transition_order: Mutex::new(()),
            clock,
            host_boot_id,
            dispatcher,
            state: Mutex::new(OwnerState::default()),
            trace: Mutex::new(TraceBuffer::default()),
        }
    }

    pub fn close_barrier(&self, now_ticks: u64) -> Result<(), LiveExecutionError> {
        self.ingest_kind(None, IngressKind::Barrier, now_ticks)
    }

    pub fn resume_after_aborted_barrier(&self) -> Result<(), LiveExecutionError> {
        let _order = self
            .transition_order
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        if let Some(active) = state.active.as_mut() {
            active.scheduler.reopen_after_barrier();
        }
        Ok(())
    }

    pub fn deactivate(&self) -> Result<(), LiveExecutionError> {
        let _order = self
            .transition_order
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        self.state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?
            .active = None;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Option<SchedulerState>, LiveExecutionError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?
            .active
            .as_ref()
            .map(|active| active.scheduler.state().clone()))
    }

    pub fn activation_epoch(&self) -> Result<Option<ActivationEpoch>, LiveExecutionError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?
            .active
            .as_ref()
            .map(|active| active.epoch))
    }

    fn ingest(&self, event: RuntimeEvent) -> Result<(), LiveExecutionError> {
        let kind = match event.kind {
            RuntimeEventKind::CleanupReceipt {
                cleanup_id,
                released,
            } => {
                self.trace
                    .lock()
                    .map_err(|_| LiveExecutionError::Poisoned)?
                    .cleanup_receipt(event.activation_epoch, &cleanup_id, released);
                return Ok(());
            }
            RuntimeEventKind::Signal {
                source,
                payload,
                source_ticks,
                reliability,
            } => IngressKind::Signal {
                source: source.stable,
                payload,
                source_ticks,
                reliability,
            },
            RuntimeEventKind::Invocation(reply) => IngressKind::Invocation { reply },
            RuntimeEventKind::TopologyFault { endpoint } => IngressKind::TopologyFault { endpoint },
            RuntimeEventKind::GrantFault => IngressKind::GrantFault,
        };
        self.ingest_kind(Some(event.activation_epoch), kind, self.clock.now_ticks())
    }

    fn ingest_kind(
        &self,
        expected_epoch: Option<ActivationEpoch>,
        kind: IngressKind,
        now_ticks: u64,
    ) -> Result<(), LiveExecutionError> {
        let _order = self
            .transition_order
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        self.ingest_ordered(expected_epoch, kind, now_ticks)
    }

    fn ingest_ordered(
        &self,
        expected_epoch: Option<ActivationEpoch>,
        kind: IngressKind,
        now_ticks: u64,
    ) -> Result<(), LiveExecutionError> {
        let commands = {
            let mut owner = self
                .state
                .lock()
                .map_err(|_| LiveExecutionError::Poisoned)?;
            let Some(active) = owner.active.as_mut() else {
                return Ok(());
            };
            if expected_epoch.is_some_and(|epoch| epoch != active.epoch) {
                return Ok(());
            }
            self.trace
                .lock()
                .map_err(|_| LiveExecutionError::Poisoned)?
                .ingress(active.epoch, &kind);
            if let IngressKind::Signal { source, .. }
            | IngressKind::TopologyFault { endpoint: source } = &kind
                && !active
                    .bindings
                    .values()
                    .any(|binding| binding.stable == *source)
            {
                return Ok(());
            }
            active.next_ordinal = active
                .next_ordinal
                .checked_add(1)
                .ok_or(LiveExecutionError::OrdinalExhausted)?;
            let ordinal = IngressOrdinal::try_new(active.next_ordinal)
                .map_err(|_| LiveExecutionError::OrdinalExhausted)?;
            let item = IngressItem {
                context: IngressContext {
                    host_boot_id: self.host_boot_id.clone(),
                    activation_epoch: active.epoch,
                    ordinal,
                },
                kind,
            };
            match active.scheduler.admit(item) {
                Ok(admission) => self
                    .trace
                    .lock()
                    .map_err(|_| LiveExecutionError::Poisoned)?
                    .admission(active.epoch, admission),
                Err(overflow) => {
                    let cleanup = translate_cleanup(active, overflow.cleanup)?;
                    drop(owner);
                    return self.dispatch_commands(vec![cleanup]);
                }
            }
            let mut commands = Vec::new();
            while let Some(step) = active
                .scheduler
                .step(now_ticks)
                .map_err(|error| LiveExecutionError::Scheduler(error.to_string()))?
            {
                for fault in &step.faults {
                    self.trace
                        .lock()
                        .map_err(|_| LiveExecutionError::Poisoned)?
                        .scheduler_fault(active.epoch, fault);
                }
                for intent in step.intents {
                    commands.push(translate_intent(active, intent)?);
                }
            }
            active
                .invocation_routes
                .retain(|id, _| active.scheduler.state().pending.contains_key(id));
            commands
        };
        self.dispatch_commands(commands)
    }

    fn dispatch_commands(&self, commands: Vec<RuntimeCommand>) -> Result<(), LiveExecutionError> {
        let mut failures = Vec::new();
        for command in commands {
            let trace_command = command.clone();
            self.trace
                .lock()
                .map_err(|_| LiveExecutionError::Poisoned)?
                .command(&trace_command);
            let invocation_id = match &command {
                RuntimeCommand::Invoke { request, .. } => Some(request.invocation_id.clone()),
                _ => None,
            };
            if let Err(failure) = self.dispatcher.dispatch(command) {
                self.trace
                    .lock()
                    .map_err(|_| LiveExecutionError::Poisoned)?
                    .dispatch_rejected(&trace_command, &failure.error.code);
                if let Some(invocation_id) = invocation_id {
                    failures.push((invocation_id, failure));
                }
            }
        }
        for (invocation_id, failure) in failures {
            self.publish_dispatch_failure(invocation_id, failure);
        }
        Ok(())
    }

    fn publish_dispatch_failure(
        &self,
        invocation_id: InvocationId,
        failure: RuntimeDispatchFailure,
    ) {
        let epoch = self.activation_epoch().ok().flatten();
        let Some(epoch) = epoch else {
            return;
        };
        let reply = if failure.possibly_sent {
            InvocationReply::Terminal {
                invocation_id,
                outcome: CommandOutcome::UnknownOutcome {
                    error: unknown_outcome(failure.error),
                    known_effects: vec![],
                },
            }
        } else {
            InvocationReply::Rejected {
                invocation_id,
                error: failure.error,
            }
        };
        let _ignored = self.ingest_ordered(
            Some(epoch),
            IngressKind::Invocation { reply },
            self.clock.now_ticks(),
        );
    }
}

impl RuntimeEventSink for LiveExecutionOwner {
    fn publish(&self, event: RuntimeEvent) {
        let _ignored = self.ingest(event);
    }
}

impl RuntimeDiagnosticsSource for LiveExecutionOwner {
    fn runtime_trace(&self) -> RuntimeTraceSnapshot {
        self.trace
            .lock()
            .map(|trace| trace.snapshot())
            .unwrap_or_else(|_| RuntimeTraceSnapshot {
                format_version: 1,
                plan_hash: None,
                schema_hashes: vec![],
                entries: vec![],
                dropped_entries: 0,
                capacity: 0,
            })
    }
}

fn unknown_outcome(mut error: DeckWeaveError) -> DeckWeaveError {
    error.category = ErrorCategory::UnknownOutcome;
    error
}
