// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered live owner that connects runtime events to the pure scheduler.

use super::trace::TraceBuffer;
use super::{
    RuntimeCommand, RuntimeDispatchFailure, RuntimeEvent, RuntimeEventKind, RuntimeEventSink,
    RuntimeIntentDispatcher,
};
use deckweave_application::{RuntimeDiagnosticsSource, RuntimeTraceSnapshot};
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
    host_boot_id: HostBootId,
    dispatcher: Arc<dyn RuntimeIntentDispatcher>,
    state: Mutex<OwnerState>,
    trace: Mutex<TraceBuffer>,
}

impl LiveExecutionOwner {
    #[must_use]
    pub fn new(host_boot_id: HostBootId, dispatcher: Arc<dyn RuntimeIntentDispatcher>) -> Self {
        Self {
            host_boot_id,
            dispatcher,
            state: Mutex::new(OwnerState::default()),
            trace: Mutex::new(TraceBuffer::default()),
        }
    }

    pub fn activate(
        &self,
        plan_hash: &str,
        graph: Option<&ExecutableGraph>,
        binding: Option<&BoundExecutionGraph>,
        limits: GraphLimits,
    ) -> Result<ActivationEpoch, LiveExecutionError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        state.next_epoch = state
            .next_epoch
            .checked_add(1)
            .ok_or(LiveExecutionError::EpochExhausted)?;
        let epoch = ActivationEpoch::try_new(state.next_epoch)
            .map_err(|_| LiveExecutionError::EpochExhausted)?;
        self.trace
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?
            .activate(plan_hash, graph, epoch, limits.maximum_trace_entries);
        let Some(graph) = graph else {
            state.active = None;
            return Ok(epoch);
        };
        let binding = binding.ok_or(LiveExecutionError::MissingGraph)?;
        let bindings = binding
            .endpoints
            .iter()
            .map(|endpoint| (endpoint.node_id.clone(), endpoint.endpoint.clone()))
            .collect::<BTreeMap<_, _>>();
        for contract in &graph.endpoint_contracts {
            if !bindings.contains_key(&contract.node_id) {
                return Err(LiveExecutionError::MissingBinding(contract.node_id.clone()));
            }
        }
        let scheduler =
            WorkspaceScheduler::new(graph.clone(), self.host_boot_id.clone(), epoch, limits)
                .map_err(|error| LiveExecutionError::Scheduler(error.to_string()))?;
        state.active = Some(ActiveExecution {
            epoch,
            graph: graph.clone(),
            scheduler,
            bindings,
            invocation_routes: BTreeMap::new(),
            next_ordinal: 0,
        });
        Ok(epoch)
    }

    pub fn close_barrier(&self, now_ticks: u64) -> Result<(), LiveExecutionError> {
        self.ingest_kind(None, IngressKind::Barrier, now_ticks)
    }

    pub fn resume_after_aborted_barrier(&self) -> Result<(), LiveExecutionError> {
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
        self.ingest_kind(Some(event.activation_epoch), kind, 0)
    }

    fn ingest_kind(
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
            if let IngressKind::Invocation { reply } = &kind
                && let Some(invocation_id) = terminal_invocation(reply)
            {
                active.invocation_routes.remove(invocation_id);
            }
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
                    return self
                        .dispatch_commands(vec![translate_cleanup(active, overflow.cleanup)?]);
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
            commands
        };
        self.dispatch_commands(commands)
    }

    fn dispatch_commands(&self, commands: Vec<RuntimeCommand>) -> Result<(), LiveExecutionError> {
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
                    self.publish_dispatch_failure(invocation_id, failure);
                }
            }
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
        let _ignored = self.ingest(RuntimeEvent {
            activation_epoch: epoch,
            kind: RuntimeEventKind::Invocation(reply),
        });
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

fn terminal_invocation(reply: &InvocationReply) -> Option<&InvocationId> {
    match reply {
        InvocationReply::Terminal { invocation_id, .. }
        | InvocationReply::Rejected { invocation_id, .. } => Some(invocation_id),
        InvocationReply::AcceptedJob { .. } => None,
    }
}

fn unknown_outcome(mut error: DeckWeaveError) -> DeckWeaveError {
    error.category = ErrorCategory::UnknownOutcome;
    error
}
