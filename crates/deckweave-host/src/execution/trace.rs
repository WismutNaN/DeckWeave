// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, payload-redacted live trace projection.

use crate::RuntimeCommand;
use deckweave_application::{RuntimeTraceEntry, RuntimeTraceEventKind, RuntimeTraceSnapshot};
use deckweave_automation::{ExecutableGraph, IngressKind, QueueAdmission, SchedulerFault};
use deckweave_domain::ids::ActivationEpoch;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

pub(super) struct TraceBuffer {
    sequence: u64,
    capacity: usize,
    dropped: u64,
    plan_hash: Option<String>,
    schema_hashes: Vec<String>,
    entries: VecDeque<RuntimeTraceEntry>,
}

impl Default for TraceBuffer {
    fn default() -> Self {
        Self {
            sequence: 0,
            capacity: 1,
            dropped: 0,
            plan_hash: None,
            schema_hashes: vec![],
            entries: VecDeque::new(),
        }
    }
}

impl TraceBuffer {
    pub(super) fn cleanup_receipt(
        &mut self,
        epoch: ActivationEpoch,
        cleanup_id: &str,
        released: bool,
    ) {
        let mut value = entry(Some(epoch), RuntimeTraceEventKind::Cleanup);
        value.correlation_id = Some(cleanup_id.into());
        value.outcome = Some(
            if released {
                "released"
            } else {
                "release_unconfirmed"
            }
            .into(),
        );
        self.push(value);
    }
    pub(super) fn activate(
        &mut self,
        plan_hash: &str,
        graph: Option<&ExecutableGraph>,
        epoch: ActivationEpoch,
        capacity: usize,
    ) {
        self.capacity = capacity.max(1);
        self.plan_hash = Some(plan_hash.into());
        self.schema_hashes = graph
            .map(|graph| {
                graph
                    .schema_contracts
                    .iter()
                    .filter_map(|contract| serde_json::to_vec(contract).ok())
                    .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
                    .collect()
            })
            .unwrap_or_default();
        self.push(entry(Some(epoch), RuntimeTraceEventKind::Activation));
    }

    pub(super) fn ingress(&mut self, epoch: ActivationEpoch, kind: &IngressKind) {
        let (trace_kind, invocation_id, outcome) = match kind {
            IngressKind::Signal { .. } => (RuntimeTraceEventKind::IngressSignal, None, None),
            IngressKind::Invocation { reply } => {
                let (id, status) = match reply {
                    deckweave_domain::automation::InvocationReply::AcceptedJob {
                        invocation_id,
                        ..
                    } => (invocation_id.to_string(), "accepted_job"),
                    deckweave_domain::automation::InvocationReply::Terminal {
                        invocation_id,
                        outcome,
                    } => (invocation_id.to_string(), outcome_status(outcome)),
                    deckweave_domain::automation::InvocationReply::Rejected {
                        invocation_id,
                        ..
                    } => (invocation_id.to_string(), "rejected"),
                };
                (
                    RuntimeTraceEventKind::IngressInvocation,
                    Some(id),
                    Some(status.into()),
                )
            }
            IngressKind::TopologyFault { .. } | IngressKind::GrantFault => {
                (RuntimeTraceEventKind::IngressFault, None, None)
            }
            IngressKind::Barrier => (RuntimeTraceEventKind::Barrier, None, None),
            IngressKind::StateUpdate { .. } | IngressKind::TimerDeadline { .. } => {
                (RuntimeTraceEventKind::IngressSignal, None, None)
            }
        };
        let mut entry = entry(Some(epoch), trace_kind);
        entry.invocation_id = invocation_id;
        entry.outcome = outcome;
        self.push(entry);
    }

    pub(super) fn admission(&mut self, epoch: ActivationEpoch, admission: QueueAdmission) {
        if admission == QueueAdmission::Coalesced {
            self.push(entry(Some(epoch), RuntimeTraceEventKind::QueueCoalesced));
        }
    }

    pub(super) fn scheduler_fault(&mut self, epoch: ActivationEpoch, fault: &SchedulerFault) {
        let mut entry = entry(Some(epoch), RuntimeTraceEventKind::IngressFault);
        entry.outcome = Some(format!("{fault:?}"));
        self.push(entry);
    }

    pub(super) fn command(&mut self, command: &RuntimeCommand) {
        let (epoch, kind, run_id, invocation_id, node_id) = match command {
            RuntimeCommand::Invoke { request, .. } => (
                request.activation_epoch,
                RuntimeTraceEventKind::IntentInvoke,
                Some(request.execution_id.to_string()),
                Some(request.invocation_id.to_string()),
                Some(request.graph_node_id.to_string()),
            ),
            RuntimeCommand::Feedback {
                run_id,
                graph_node_id,
                activation_epoch,
                ..
            } => (
                *activation_epoch,
                RuntimeTraceEventKind::IntentFeedback,
                Some(run_id.to_string()),
                None,
                Some(graph_node_id.to_string()),
            ),
            RuntimeCommand::Cancel {
                activation_epoch,
                invocation_id,
                ..
            } => (
                *activation_epoch,
                RuntimeTraceEventKind::IntentCancel,
                None,
                Some(invocation_id.to_string()),
                None,
            ),
            RuntimeCommand::CleanupRun {
                activation_epoch,
                run_id,
                ..
            } => (
                *activation_epoch,
                RuntimeTraceEventKind::Cleanup,
                Some(run_id.to_string()),
                None,
                None,
            ),
            RuntimeCommand::CleanupAll {
                activation_epoch, ..
            } => (
                *activation_epoch,
                RuntimeTraceEventKind::Cleanup,
                None,
                None,
                None,
            ),
        };
        let mut entry = entry(Some(epoch), kind);
        entry.run_id = run_id;
        entry.invocation_id = invocation_id;
        entry.graph_node_id = node_id;
        self.push(entry);
    }

    pub(super) fn dispatch_rejected(&mut self, command: &RuntimeCommand, code: &str) {
        let epoch = command_epoch(command);
        let mut entry = entry(Some(epoch), RuntimeTraceEventKind::DispatchRejected);
        entry.outcome = Some(code.into());
        self.push(entry);
    }

    pub(super) fn snapshot(&self) -> RuntimeTraceSnapshot {
        RuntimeTraceSnapshot {
            format_version: 1,
            plan_hash: self.plan_hash.clone(),
            schema_hashes: self.schema_hashes.clone(),
            entries: self.entries.iter().cloned().collect(),
            dropped_entries: self.dropped,
            capacity: self.capacity,
        }
    }

    fn push(&mut self, mut entry: RuntimeTraceEntry) {
        self.sequence = self.sequence.saturating_add(1);
        entry.sequence = self.sequence;
        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.entries.push_back(entry);
    }
}

fn entry(epoch: Option<ActivationEpoch>, kind: RuntimeTraceEventKind) -> RuntimeTraceEntry {
    RuntimeTraceEntry {
        sequence: 0,
        activation_epoch: epoch.map(ActivationEpoch::get),
        kind,
        run_id: None,
        invocation_id: None,
        graph_node_id: None,
        correlation_id: None,
        causation_id: None,
        outcome: None,
    }
}

fn command_epoch(command: &RuntimeCommand) -> ActivationEpoch {
    match command {
        RuntimeCommand::Invoke { request, .. } => request.activation_epoch,
        RuntimeCommand::Feedback {
            activation_epoch, ..
        }
        | RuntimeCommand::Cancel {
            activation_epoch, ..
        }
        | RuntimeCommand::CleanupRun {
            activation_epoch, ..
        }
        | RuntimeCommand::CleanupAll {
            activation_epoch, ..
        } => *activation_epoch,
    }
}

fn outcome_status(outcome: &deckweave_domain::automation::CommandOutcome) -> &'static str {
    match outcome {
        deckweave_domain::automation::CommandOutcome::Succeeded { .. } => "succeeded",
        deckweave_domain::automation::CommandOutcome::Failed { .. } => "failed",
        deckweave_domain::automation::CommandOutcome::Cancelled { .. } => "cancelled",
        deckweave_domain::automation::CommandOutcome::UnknownOutcome { .. } => "unknown_outcome",
    }
}
