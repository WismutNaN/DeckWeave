// SPDX-License-Identifier: GPL-3.0-or-later
//! Isolated runner over the same `WorkspaceScheduler` used by live Host execution.

use super::{
    LatencyPercentiles, SimulationError, SimulationFixture, SimulationLatency, SimulationReport,
    SimulationStatus,
    model::SIMULATION_FIXTURE_VERSION,
    recording::RecordingDispatcher,
    trace::{TraceContext, TraceRecorder},
};
use crate::{
    CompiledExecutionPlan, GraphLimits, IngressContext, IngressItem, IngressKind, SchedulerIntent,
    WorkspaceScheduler,
};
use deckweave_domain::ids::{ActivationEpoch, HostBootId, IngressOrdinal, InvocationId};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Instant};

#[derive(Clone, Default)]
struct OwnedTraceContext {
    correlation_id: Option<String>,
    causation_id: Option<String>,
}

impl OwnedTraceContext {
    fn borrowed(&self) -> TraceContext<'_> {
        TraceContext {
            correlation_id: self.correlation_id.as_deref(),
            causation_id: self.causation_id.as_deref(),
        }
    }
}

pub struct SimulationRunner {
    plan: CompiledExecutionPlan,
    limits: GraphLimits,
}

impl SimulationRunner {
    pub fn new(plan: CompiledExecutionPlan, limits: GraphLimits) -> Result<Self, SimulationError> {
        limits
            .validate()
            .map_err(|error| SimulationError::Scheduler(error.to_string()))?;
        if plan.executable_graph().is_none() {
            return Err(SimulationError::MissingGraph);
        }
        Ok(Self { plan, limits })
    }

    pub fn run(&self, fixture: &SimulationFixture) -> Result<SimulationReport, SimulationError> {
        self.validate_fixture(fixture)?;
        let graph = self
            .plan
            .executable_graph()
            .ok_or(SimulationError::MissingGraph)?;
        let host_boot_id = HostBootId::try_new("simulation")
            .map_err(|error| SimulationError::Identity(error.to_string()))?;
        let epoch = ActivationEpoch::try_new(1)
            .map_err(|error| SimulationError::Identity(error.to_string()))?;
        let mut scheduler =
            WorkspaceScheduler::new(graph.clone(), host_boot_id.clone(), epoch, self.limits)
                .map_err(|error| SimulationError::Scheduler(error.to_string()))?;
        let mut trace = TraceRecorder::new(
            plan_hash(&self.plan)?,
            schema_hashes(graph)?,
            fixture.random_seed,
            self.limits.maximum_trace_entries,
        );
        let mut recorder = RecordingDispatcher::new(self.limits.maximum_trace_entries);
        let mut ordinal = 0_u64;
        let mut ingress_samples = Vec::new();
        let mut queue_samples = Vec::new();
        let mut step_samples = Vec::new();
        let mut provider_samples = Vec::new();
        let mut invoked_at = BTreeMap::<InvocationId, u64>::new();
        let mut ingress_started = BTreeMap::<u64, Instant>::new();
        let mut trace_contexts = BTreeMap::<u64, OwnedTraceContext>::new();

        let mut cursor = 0;
        while cursor < fixture.events.len() {
            let ticks = fixture.events[cursor].at_ticks;
            while cursor < fixture.events.len() && fixture.events[cursor].at_ticks == ticks {
                let event = &fixture.events[cursor];
                ordinal = ordinal
                    .checked_add(1)
                    .ok_or_else(|| SimulationError::Identity("ordinal exhausted".into()))?;
                let ordinal_id = IngressOrdinal::try_new(ordinal)
                    .map_err(|error| SimulationError::Identity(error.to_string()))?;
                let trace_context = OwnedTraceContext {
                    correlation_id: event.correlation_id.clone(),
                    causation_id: event.causation_id.clone(),
                };
                trace.ingress(ticks, epoch, &event.kind, trace_context.borrowed());
                completion_latency(&event.kind, ticks, &mut invoked_at, &mut provider_samples);
                let admitted_at = Instant::now();
                let admission = scheduler.admit(IngressItem {
                    context: IngressContext {
                        host_boot_id: host_boot_id.clone(),
                        activation_epoch: epoch,
                        ordinal: ordinal_id,
                    },
                    kind: event.kind.clone(),
                });
                match admission {
                    Ok(value) => {
                        trace.admission(ticks, epoch, value, trace_context.borrowed());
                        ingress_started.insert(ordinal, admitted_at);
                        trace_contexts.insert(ordinal, trace_context);
                    }
                    Err(overflow) => {
                        trace.overload(ticks, epoch, "ingress_queue_overflow");
                        trace.intent(ticks, epoch, &overflow.cleanup, trace_context.borrowed());
                        recorder.record(overflow.cleanup);
                        recorder.mark_ingress_overload();
                    }
                }
                cursor += 1;
            }

            loop {
                let step_started = Instant::now();
                let Some(step) = scheduler
                    .step(ticks)
                    .map_err(|error| SimulationError::Scheduler(error.to_string()))?
                else {
                    break;
                };
                step_samples.push(nanos(step_started.elapsed()));
                let consumed_ordinal = step.ingress.ordinal.get();
                let context = trace_contexts.remove(&consumed_ordinal).unwrap_or_default();
                let admitted_at = ingress_started
                    .remove(&consumed_ordinal)
                    .unwrap_or(step_started);
                queue_samples.push(nanos(step_started.duration_since(admitted_at)));
                for fault in &step.faults {
                    trace.fault(ticks, epoch, fault, context.borrowed());
                }
                for intent in step.intents {
                    if let SchedulerIntent::Invoke { invocation_id, .. } = &intent {
                        invoked_at.insert(invocation_id.clone(), ticks);
                    }
                    trace.intent(ticks, epoch, &intent, context.borrowed());
                    ingress_samples.push(nanos(admitted_at.elapsed()));
                    let was_overloaded = recorder.overloaded();
                    if !recorder.record(intent) && !was_overloaded {
                        trace.overload(ticks, epoch, "recording_dispatcher_overflow");
                    }
                }
            }
            // Coalesced ingress entries were replaced before consumption. No
            // metadata may survive into the next fake-clock tick.
            ingress_started.clear();
            trace_contexts.clear();
        }
        let status = if recorder.overloaded() {
            SimulationStatus::Overloaded
        } else if scheduler.state().pending.is_empty() {
            SimulationStatus::Completed
        } else {
            SimulationStatus::Inconclusive
        };
        let (recorded_intents, dropped_intents) = recorder.finish();
        Ok(SimulationReport {
            status,
            final_state: scheduler.state().clone(),
            recorded_intents,
            dropped_intents,
            trace: trace.finish(),
            latency: SimulationLatency {
                ingress_to_dispatch_ns: LatencyPercentiles::from_samples(ingress_samples),
                queue_wait_ns: LatencyPercentiles::from_samples(queue_samples),
                scheduler_step_ns: LatencyPercentiles::from_samples(step_samples),
                provider_completion_ticks: LatencyPercentiles::from_samples(provider_samples),
            },
        })
    }

    fn validate_fixture(&self, fixture: &SimulationFixture) -> Result<(), SimulationError> {
        if fixture.format_version != SIMULATION_FIXTURE_VERSION {
            return Err(SimulationError::UnsupportedFixture(fixture.format_version));
        }
        if fixture.events.len() > self.limits.maximum_trace_entries {
            return Err(SimulationError::EventBudget {
                actual: fixture.events.len(),
                maximum: self.limits.maximum_trace_entries,
            });
        }
        if fixture
            .events
            .windows(2)
            .any(|events| events[0].at_ticks > events[1].at_ticks)
        {
            return Err(SimulationError::UnorderedEvents);
        }
        if fixture.events.iter().any(|event| {
            !valid_trace_id(event.correlation_id.as_deref())
                || !valid_trace_id(event.causation_id.as_deref())
        }) {
            return Err(SimulationError::InvalidTraceContext);
        }
        Ok(())
    }
}

fn completion_latency(
    kind: &IngressKind,
    ticks: u64,
    invoked_at: &mut BTreeMap<InvocationId, u64>,
    samples: &mut Vec<u64>,
) {
    let invocation_id = match kind {
        IngressKind::Invocation {
            reply: deckweave_domain::automation::InvocationReply::Terminal { invocation_id, .. },
        }
        | IngressKind::Invocation {
            reply: deckweave_domain::automation::InvocationReply::Rejected { invocation_id, .. },
        } => Some(invocation_id),
        _ => None,
    };
    if let Some(started) = invocation_id.and_then(|id| invoked_at.remove(id)) {
        samples.push(ticks.saturating_sub(started));
    }
}

fn plan_hash(plan: &CompiledExecutionPlan) -> Result<String, SimulationError> {
    serde_json::to_vec(plan)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| SimulationError::Hash(error.to_string()))
}

fn schema_hashes(graph: &crate::ExecutableGraph) -> Result<Vec<String>, SimulationError> {
    let mut hashes = graph
        .schema_contracts
        .iter()
        .map(|contract| {
            serde_json::to_vec(contract)
                .map(|bytes| {
                    format!(
                        "{}@{}.{}:{:x}",
                        contract.schema.id,
                        contract.schema.version.major,
                        contract.schema.version.minor,
                        Sha256::digest(bytes)
                    )
                })
                .map_err(|error| SimulationError::Hash(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    hashes.sort();
    Ok(hashes)
}

fn nanos(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn valid_trace_id(value: Option<&str>) -> bool {
    value.is_none_or(|value| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-:/".contains(&byte))
    })
}
