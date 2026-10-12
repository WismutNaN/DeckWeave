// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned bounded trace produced by simulation/replay.

use crate::{IngressKind, QueueAdmission, SchedulerFault, SchedulerIntent};
use deckweave_domain::ids::ActivationEpoch;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const SIMULATION_TRACE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationTraceKind {
    Ingress,
    Coalesced,
    Intent,
    Fault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationTraceEntry {
    pub sequence: u64,
    pub ticks: u64,
    pub activation_epoch: u64,
    pub kind: SimulationTraceKind,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub invocation_id: Option<String>,
    pub graph_node_id: Option<String>,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationTrace {
    pub format_version: u32,
    pub plan_hash: String,
    pub schema_hashes: Vec<String>,
    pub random_seed: u64,
    pub entries: Vec<SimulationTraceEntry>,
    pub dropped_entries: u64,
    pub capacity: usize,
}

pub(super) struct TraceRecorder {
    trace: SimulationTrace,
    entries: VecDeque<SimulationTraceEntry>,
    next_sequence: u64,
}

#[derive(Clone, Copy)]
pub(super) struct TraceContext<'a> {
    pub(super) correlation_id: Option<&'a str>,
    pub(super) causation_id: Option<&'a str>,
}

impl TraceRecorder {
    pub(super) fn new(
        plan_hash: String,
        schema_hashes: Vec<String>,
        random_seed: u64,
        capacity: usize,
    ) -> Self {
        Self {
            trace: SimulationTrace {
                format_version: SIMULATION_TRACE_VERSION,
                plan_hash,
                schema_hashes,
                random_seed,
                entries: vec![],
                dropped_entries: 0,
                capacity: capacity.max(1),
            },
            entries: VecDeque::new(),
            next_sequence: 0,
        }
    }

    pub(super) fn ingress(
        &mut self,
        ticks: u64,
        epoch: ActivationEpoch,
        kind: &IngressKind,
        context: TraceContext<'_>,
    ) {
        let (invocation_id, outcome) = match kind {
            IngressKind::Invocation { reply } => match reply {
                deckweave_domain::automation::InvocationReply::AcceptedJob {
                    invocation_id,
                    ..
                } => (Some(invocation_id.to_string()), Some("accepted_job".into())),
                deckweave_domain::automation::InvocationReply::Terminal {
                    invocation_id,
                    outcome,
                } => (
                    Some(invocation_id.to_string()),
                    Some(outcome_name(outcome).into()),
                ),
                deckweave_domain::automation::InvocationReply::Rejected {
                    invocation_id, ..
                } => (Some(invocation_id.to_string()), Some("rejected".into())),
            },
            _ => (None, None),
        };
        let mut entry = self.entry(ticks, epoch, SimulationTraceKind::Ingress);
        entry.correlation_id = context.correlation_id.map(str::to_owned);
        entry.causation_id = context.causation_id.map(str::to_owned);
        entry.invocation_id = invocation_id;
        entry.outcome = outcome;
        self.push(entry);
    }

    pub(super) fn admission(
        &mut self,
        ticks: u64,
        epoch: ActivationEpoch,
        value: QueueAdmission,
        context: TraceContext<'_>,
    ) {
        if value == QueueAdmission::Coalesced {
            let mut entry = self.entry(ticks, epoch, SimulationTraceKind::Coalesced);
            entry.correlation_id = context.correlation_id.map(str::to_owned);
            entry.causation_id = context.causation_id.map(str::to_owned);
            self.push(entry);
        }
    }

    pub(super) fn fault(
        &mut self,
        ticks: u64,
        epoch: ActivationEpoch,
        fault: &SchedulerFault,
        context: TraceContext<'_>,
    ) {
        let mut entry = self.entry(ticks, epoch, SimulationTraceKind::Fault);
        entry.correlation_id = context.correlation_id.map(str::to_owned);
        entry.causation_id = context.causation_id.map(str::to_owned);
        entry.outcome = Some(format!("{fault:?}"));
        self.push(entry);
    }

    pub(super) fn overload(&mut self, ticks: u64, epoch: ActivationEpoch, reason: &str) {
        let mut entry = self.entry(ticks, epoch, SimulationTraceKind::Fault);
        entry.outcome = Some(reason.to_owned());
        self.push(entry);
    }

    pub(super) fn intent(
        &mut self,
        ticks: u64,
        epoch: ActivationEpoch,
        intent: &SchedulerIntent,
        context: TraceContext<'_>,
    ) {
        let mut entry = self.entry(ticks, epoch, SimulationTraceKind::Intent);
        entry.correlation_id = context.correlation_id.map(str::to_owned);
        entry.causation_id = context.causation_id.map(str::to_owned);
        match intent {
            SchedulerIntent::Invoke {
                invocation_id,
                node_id,
                ..
            } => {
                entry.invocation_id = Some(invocation_id.to_string());
                entry.graph_node_id = Some(node_id.to_string());
                entry.outcome = Some("invoke".into());
            }
            SchedulerIntent::Feedback { node_id, .. } => {
                entry.graph_node_id = Some(node_id.to_string());
                entry.outcome = Some("feedback".into());
            }
            SchedulerIntent::CancelInvocation { invocation_id, .. } => {
                entry.invocation_id = Some(invocation_id.to_string());
                entry.outcome = Some("cancel".into());
            }
            SchedulerIntent::CleanupRun { .. } | SchedulerIntent::CleanupAll { .. } => {
                entry.outcome = Some("cleanup".into());
            }
        }
        self.push(entry);
    }

    pub(super) fn finish(mut self) -> SimulationTrace {
        self.trace.entries = self.entries.into_iter().collect();
        self.trace
    }

    fn entry(
        &self,
        ticks: u64,
        epoch: ActivationEpoch,
        kind: SimulationTraceKind,
    ) -> SimulationTraceEntry {
        SimulationTraceEntry {
            sequence: 0,
            ticks,
            activation_epoch: epoch.get(),
            kind,
            correlation_id: None,
            causation_id: None,
            invocation_id: None,
            graph_node_id: None,
            outcome: None,
        }
    }

    fn push(&mut self, mut entry: SimulationTraceEntry) {
        self.next_sequence = self.next_sequence.saturating_add(1);
        entry.sequence = self.next_sequence;
        while self.entries.len() >= self.trace.capacity {
            self.entries.pop_front();
            self.trace.dropped_entries = self.trace.dropped_entries.saturating_add(1);
        }
        self.entries.push_back(entry);
    }
}

fn outcome_name(outcome: &deckweave_domain::automation::CommandOutcome) -> &'static str {
    match outcome {
        deckweave_domain::automation::CommandOutcome::Succeeded { .. } => "succeeded",
        deckweave_domain::automation::CommandOutcome::Failed { .. } => "failed",
        deckweave_domain::automation::CommandOutcome::Cancelled { .. } => "cancelled",
        deckweave_domain::automation::CommandOutcome::UnknownOutcome { .. } => "unknown_outcome",
    }
}
