// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_domain::{
    automation::{CommandOutcome, InvocationReply},
    endpoint::StableEndpointRef,
    ids::{
        ActivationEpoch, ExecutionId, GraphNodeId, HostBootId, IngressOrdinal, InvocationId, JobId,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

use crate::GraphLimitKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressContext {
    pub host_boot_id: HostBootId,
    pub activation_epoch: ActivationEpoch,
    pub ordinal: IngressOrdinal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngressReliability {
    Reliable,
    Coalescible,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IngressItem {
    pub context: IngressContext,
    pub kind: IngressKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum IngressKind {
    Signal {
        source: StableEndpointRef,
        payload: Value,
        source_ticks: u64,
        reliability: IngressReliability,
    },
    StateUpdate {
        node_id: GraphNodeId,
        value: Value,
    },
    TimerDeadline {
        invocation_id: InvocationId,
    },
    Invocation {
        reply: InvocationReply,
    },
    TopologyFault {
        endpoint: StableEndpointRef,
    },
    GrantFault,
    Barrier,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingInvocation {
    pub invocation_id: InvocationId,
    pub run_id: ExecutionId,
    pub node_id: GraphNodeId,
    pub deadline_ticks: u64,
    pub accepted_job: Option<JobId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerState {
    pub run_results: BTreeMap<ExecutionId, BTreeMap<GraphNodeId, Value>>,
    pub host_boot_id: HostBootId,
    pub activation_epoch: ActivationEpoch,
    pub last_ordinal: Option<IngressOrdinal>,
    pub closed: bool,
    /// A grant/topology fault requires a new activation, even if an apply aborts.
    pub invalidated: bool,
    pub values: BTreeMap<GraphNodeId, Value>,
    pub source_ticks: BTreeMap<StableEndpointRef, u64>,
    pub pending: BTreeMap<InvocationId, PendingInvocation>,
    pub completed: BTreeMap<InvocationId, CommandOutcome>,
    pub completion_order: VecDeque<InvocationId>,
    pub active_runs: BTreeSet<ExecutionId>,
    pub next_run_sequence: u64,
    pub next_invocation_sequence: u64,
}

impl SchedulerState {
    #[must_use]
    pub fn new(host_boot_id: HostBootId, activation_epoch: ActivationEpoch) -> Self {
        Self {
            run_results: BTreeMap::new(),
            host_boot_id,
            activation_epoch,
            last_ordinal: None,
            closed: false,
            invalidated: false,
            values: BTreeMap::new(),
            source_ticks: BTreeMap::new(),
            pending: BTreeMap::new(),
            completed: BTreeMap::new(),
            completion_order: VecDeque::new(),
            active_runs: BTreeSet::new(),
            next_run_sequence: 1,
            next_invocation_sequence: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerIntent {
    Invoke {
        invocation_id: InvocationId,
        run_id: ExecutionId,
        node_id: GraphNodeId,
        target: StableEndpointRef,
        input: Value,
        deadline_ticks: u64,
    },
    Feedback {
        run_id: ExecutionId,
        node_id: GraphNodeId,
        target: StableEndpointRef,
        value: Value,
    },
    CancelInvocation {
        invocation_id: InvocationId,
        job_id: Option<JobId>,
        reason: String,
    },
    CleanupRun {
        run_id: ExecutionId,
        reason: String,
    },
    CleanupAll {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerFault {
    StaleIngress,
    UnknownSource,
    UnknownState,
    DuplicateTerminal,
    ConflictingTerminal,
    ConflictingAcceptance,
    UnknownInvocation,
    MalformedInput,
    MalformedResult,
    OversizedInput,
    OversizedResult,
    DeadlineExceeded,
    TopologyChanged,
    GrantRevoked,
    BarrierClosed,
    BudgetExceeded(GraphLimitKind),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerTransition {
    pub state: SchedulerState,
    pub intents: Vec<SchedulerIntent>,
    pub faults: Vec<SchedulerFault>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchedulerTransitionError {
    #[error("ingress ordinal {actual} is not greater than previous ordinal {previous}")]
    IngressOutOfOrder { previous: u64, actual: u64 },
    #[error("compiled graph invariant failed: {0}")]
    InvalidPlan(String),
    #[error("scheduler could not create an internal identity: {0}")]
    InvalidIdentity(String),
}
