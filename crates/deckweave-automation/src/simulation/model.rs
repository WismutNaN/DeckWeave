// SPDX-License-Identifier: GPL-3.0-or-later

use super::{SimulationLatency, SimulationTrace};
use crate::{IngressKind, SchedulerIntent, SchedulerState};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SIMULATION_FIXTURE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationEvent {
    pub at_ticks: u64,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub kind: IngressKind,
}

impl SimulationEvent {
    #[must_use]
    pub fn new(at_ticks: u64, kind: IngressKind) -> Self {
        Self {
            at_ticks,
            correlation_id: None,
            causation_id: None,
            kind,
        }
    }

    #[must_use]
    pub fn with_trace_context(
        mut self,
        correlation_id: impl Into<String>,
        causation_id: impl Into<String>,
    ) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self.causation_id = Some(causation_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationFixture {
    pub format_version: u32,
    pub random_seed: u64,
    pub events: Vec<SimulationEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationStatus {
    Completed,
    Inconclusive,
    Overloaded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationReport {
    pub status: SimulationStatus,
    pub final_state: SchedulerState,
    pub recorded_intents: Vec<SchedulerIntent>,
    pub dropped_intents: u64,
    pub trace: SimulationTrace,
    pub latency: SimulationLatency,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SimulationError {
    #[error("unsupported simulation fixture version {0}")]
    UnsupportedFixture(u32),
    #[error("simulation fixture events are not ordered by monotonic ticks")]
    UnorderedEvents,
    #[error("simulation fixture exceeds event budget: {actual} > {maximum}")]
    EventBudget { actual: usize, maximum: usize },
    #[error("simulation trace context contains an invalid identifier")]
    InvalidTraceContext,
    #[error("compiled plan has no executable graph")]
    MissingGraph,
    #[error("simulation identity is invalid: {0}")]
    Identity(String),
    #[error("simulation scheduler failed: {0}")]
    Scheduler(String),
    #[error("simulation plan hashing failed: {0}")]
    Hash(String),
}
