// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/simulation_plan.rs"]
mod simulation_plan;

use deckweave_automation::{
    GraphLimits, IngressKind, IngressReliability, SIMULATION_FIXTURE_VERSION,
    SIMULATION_TRACE_VERSION, SchedulerIntent, SimulationError, SimulationEvent, SimulationFixture,
    SimulationRunner, SimulationStatus, SimulationTraceKind,
};
use deckweave_domain::{
    automation::{CommandOutcome, InvocationReply},
    ids::{GraphNodeId, InvocationId, JobId},
};
use serde_json::json;
use simulation_plan::{compiled_plan, source_endpoint};
use std::error::Error;

#[test]
fn replay_uses_compiled_plan_and_redacts_payloads_from_versioned_trace()
-> Result<(), Box<dyn Error>> {
    let limits = GraphLimits::default();
    let runner = SimulationRunner::new(compiled_plan(limits)?, limits)?;
    let fixture = SimulationFixture {
        format_version: SIMULATION_FIXTURE_VERSION,
        random_seed: 41,
        events: vec![
            signal(10, "private-source", IngressReliability::Reliable)?
                .with_trace_context("corr-1", "cause-1"),
            SimulationEvent::new(
                15,
                IngressKind::Invocation {
                    reply: InvocationReply::AcceptedJob {
                        invocation_id: invocation(1)?,
                        job_id: JobId::try_new("job-1")?,
                    },
                },
            ),
            terminal(30, 1, "private-result")?,
        ],
    };
    let fixture_json = serde_json::to_string(&fixture)?;
    let fixture: SimulationFixture = serde_json::from_str(&fixture_json)?;

    let report = runner.run(&fixture)?;

    assert_eq!(report.status, SimulationStatus::Completed);
    assert_eq!(report.dropped_intents, 0);
    assert!(report.final_state.pending.is_empty());
    assert_eq!(
        report
            .final_state
            .values
            .get(&GraphNodeId::try_new("state")?),
        Some(&json!("private-result"))
    );
    assert!(report.recorded_intents.iter().any(|intent| matches!(
        intent,
        SchedulerIntent::Feedback { value, .. } if value == &json!("private-result")
    )));
    assert_eq!(report.trace.format_version, SIMULATION_TRACE_VERSION);
    assert!(!report.trace.plan_hash.is_empty());
    assert!(!report.trace.schema_hashes.is_empty());
    assert_eq!(report.trace.random_seed, 41);
    assert!(report.trace.entries.iter().any(|entry| {
        entry.kind == SimulationTraceKind::Intent
            && entry.correlation_id.as_deref() == Some("corr-1")
            && entry.causation_id.as_deref() == Some("cause-1")
    }));
    let trace_json = serde_json::to_string(&report.trace)?;
    assert!(!trace_json.contains("private-source"));
    assert!(!trace_json.contains("private-result"));
    assert_eq!(report.latency.provider_completion_ticks.samples, 1);
    assert_eq!(report.latency.provider_completion_ticks.p50, 20);
    Ok(())
}

#[test]
fn missing_external_outcome_is_inconclusive_and_never_calls_a_live_dispatcher()
-> Result<(), Box<dyn Error>> {
    let limits = GraphLimits::default();
    let runner = SimulationRunner::new(compiled_plan(limits)?, limits)?;
    let report = runner.run(&SimulationFixture {
        format_version: SIMULATION_FIXTURE_VERSION,
        random_seed: 1,
        events: vec![signal(10, true, IngressReliability::Reliable)?],
    })?;

    assert_eq!(report.status, SimulationStatus::Inconclusive);
    assert_eq!(report.final_state.pending.len(), 1);
    assert_eq!(
        report
            .recorded_intents
            .iter()
            .filter(|intent| matches!(intent, SchedulerIntent::Invoke { .. }))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn fixture_version_order_budget_and_trace_ids_are_validated() -> Result<(), Box<dyn Error>> {
    let limits = GraphLimits {
        maximum_trace_entries: 2,
        ..GraphLimits::default()
    };
    let runner = SimulationRunner::new(compiled_plan(limits)?, limits)?;

    for (fixture, expected) in [
        (
            SimulationFixture {
                format_version: 99,
                random_seed: 0,
                events: vec![],
            },
            SimulationError::UnsupportedFixture(99),
        ),
        (
            SimulationFixture {
                format_version: SIMULATION_FIXTURE_VERSION,
                random_seed: 0,
                events: vec![
                    signal(20, true, IngressReliability::Reliable)?,
                    signal(10, true, IngressReliability::Reliable)?,
                ],
            },
            SimulationError::UnorderedEvents,
        ),
        (
            SimulationFixture {
                format_version: SIMULATION_FIXTURE_VERSION,
                random_seed: 0,
                events: vec![
                    signal(10, true, IngressReliability::Reliable)?,
                    signal(20, true, IngressReliability::Reliable)?,
                    signal(30, true, IngressReliability::Reliable)?,
                ],
            },
            SimulationError::EventBudget {
                actual: 3,
                maximum: 2,
            },
        ),
        (
            SimulationFixture {
                format_version: SIMULATION_FIXTURE_VERSION,
                random_seed: 0,
                events: vec![
                    signal(10, true, IngressReliability::Reliable)?
                        .with_trace_context("private text is not an id", "cause"),
                ],
            },
            SimulationError::InvalidTraceContext,
        ),
    ] {
        assert_eq!(runner.run(&fixture), Err(expected));
    }
    Ok(())
}

#[test]
fn fixture_drives_timer_grant_and_topology_faults_with_fake_time() -> Result<(), Box<dyn Error>> {
    let limits = GraphLimits::default();
    let runner = SimulationRunner::new(compiled_plan(limits)?, limits)?;
    let timeout = runner.run(&SimulationFixture {
        format_version: SIMULATION_FIXTURE_VERSION,
        random_seed: 2,
        events: vec![
            signal(10, true, IngressReliability::Reliable)?,
            SimulationEvent::new(
                1_010,
                IngressKind::TimerDeadline {
                    invocation_id: invocation(1)?,
                },
            ),
        ],
    })?;
    let first_invocation = invocation(1)?;
    assert_eq!(timeout.status, SimulationStatus::Completed);
    assert!(timeout.recorded_intents.iter().any(|intent| matches!(
        intent,
        SchedulerIntent::CancelInvocation { invocation_id, .. }
            if invocation_id == &first_invocation
    )));

    for fault in [
        IngressKind::GrantFault,
        IngressKind::TopologyFault {
            endpoint: source_endpoint()?,
        },
    ] {
        let report = runner.run(&SimulationFixture {
            format_version: SIMULATION_FIXTURE_VERSION,
            random_seed: 3,
            events: vec![
                signal(10, true, IngressReliability::Reliable)?,
                SimulationEvent::new(20, fault),
            ],
        })?;
        assert_eq!(report.status, SimulationStatus::Completed);
        assert!(report.final_state.pending.is_empty());
    }
    Ok(())
}

#[test]
fn coalescing_and_ingress_overload_are_visible_and_bounded() -> Result<(), Box<dyn Error>> {
    let limits = GraphLimits {
        maximum_queued_events: 1,
        maximum_trace_entries: 8,
        ..GraphLimits::default()
    };
    let runner = SimulationRunner::new(compiled_plan(limits)?, limits)?;

    let coalesced = runner.run(&SimulationFixture {
        format_version: SIMULATION_FIXTURE_VERSION,
        random_seed: 4,
        events: vec![
            signal(10, "first", IngressReliability::Coalescible)?,
            signal(10, "second", IngressReliability::Coalescible)?,
        ],
    })?;
    assert_eq!(coalesced.status, SimulationStatus::Inconclusive);
    assert!(
        coalesced
            .trace
            .entries
            .iter()
            .any(|entry| entry.kind == SimulationTraceKind::Coalesced)
    );
    assert_eq!(coalesced.final_state.pending.len(), 1);

    let overloaded = runner.run(&SimulationFixture {
        format_version: SIMULATION_FIXTURE_VERSION,
        random_seed: 5,
        events: vec![
            signal(10, true, IngressReliability::Reliable)?,
            signal(10, true, IngressReliability::Reliable)?,
        ],
    })?;
    assert_eq!(overloaded.status, SimulationStatus::Overloaded);
    assert!(overloaded.recorded_intents.iter().any(|intent| matches!(
        intent,
        SchedulerIntent::CleanupAll { reason } if reason.contains("overflow")
    )));
    assert!(overloaded.trace.entries.len() <= overloaded.trace.capacity);
    Ok(())
}

#[test]
fn saturated_recording_sink_fails_closed_without_unbounded_growth() -> Result<(), Box<dyn Error>> {
    let limits = GraphLimits {
        maximum_trace_entries: 6,
        ..GraphLimits::default()
    };
    let runner = SimulationRunner::new(compiled_plan(limits)?, limits)?;
    let report = runner.run(&paired_fixture(3)?)?;

    assert_eq!(report.status, SimulationStatus::Overloaded);
    assert!(report.dropped_intents > 0);
    assert!(report.recorded_intents.len() <= limits.maximum_trace_entries);
    assert!(report.recorded_intents.iter().any(|intent| matches!(
        intent,
        SchedulerIntent::CleanupAll { reason } if reason.contains("recording dispatcher")
    )));
    assert!(report.trace.entries.len() <= limits.maximum_trace_entries);
    assert!(report.trace.dropped_entries > 0);
    Ok(())
}

#[test]
fn load_fixture_is_repeatable_and_reports_percentile_baselines() -> Result<(), Box<dyn Error>> {
    let limits = GraphLimits {
        maximum_trace_entries: 2_048,
        ..GraphLimits::default()
    };
    let runner = SimulationRunner::new(compiled_plan(limits)?, limits)?;
    let fixture = paired_fixture(250)?;

    let first = runner.run(&fixture)?;
    let second = runner.run(&fixture)?;

    if std::env::var_os("DECKWEAVE_PRINT_LATENCY").is_some() {
        println!("simulation latency baseline: {:?}", first.latency);
    }

    assert_eq!(first.status, SimulationStatus::Completed);
    assert_eq!(first.final_state, second.final_state);
    assert_eq!(first.recorded_intents, second.recorded_intents);
    assert_eq!(first.trace, second.trace);
    assert_eq!(first.dropped_intents, 0);
    assert!(first.trace.entries.len() <= limits.maximum_trace_entries);
    assert!(first.final_state.pending.is_empty());
    assert!(first.final_state.active_runs.is_empty());
    assert_eq!(first.latency.provider_completion_ticks.samples, 250);
    for summary in [
        &first.latency.ingress_to_dispatch_ns,
        &first.latency.queue_wait_ns,
        &first.latency.scheduler_step_ns,
        &first.latency.provider_completion_ticks,
    ] {
        assert!(summary.samples > 0);
        assert!(summary.p50 <= summary.p95);
        assert!(summary.p95 <= summary.p99);
    }
    Ok(())
}

fn paired_fixture(pairs: u64) -> Result<SimulationFixture, Box<dyn Error>> {
    let mut events = Vec::with_capacity((pairs * 2) as usize);
    for sequence in 1..=pairs {
        let signal_ticks = sequence * 10;
        events.push(signal(signal_ticks, true, IngressReliability::Reliable)?);
        events.push(terminal(signal_ticks + 1, sequence, "ok")?);
    }
    Ok(SimulationFixture {
        format_version: SIMULATION_FIXTURE_VERSION,
        random_seed: 99,
        events,
    })
}

fn signal(
    ticks: u64,
    value: impl serde::Serialize,
    reliability: IngressReliability,
) -> Result<SimulationEvent, Box<dyn Error>> {
    Ok(SimulationEvent::new(
        ticks,
        IngressKind::Signal {
            source: source_endpoint()?,
            payload: serde_json::to_value(value)?,
            source_ticks: ticks,
            reliability,
        },
    ))
}

fn terminal(ticks: u64, sequence: u64, result: &str) -> Result<SimulationEvent, Box<dyn Error>> {
    Ok(SimulationEvent::new(
        ticks,
        IngressKind::Invocation {
            reply: InvocationReply::Terminal {
                invocation_id: invocation(sequence)?,
                outcome: CommandOutcome::Succeeded {
                    result: json!(result),
                },
            },
        },
    ))
}

fn invocation(sequence: u64) -> Result<InvocationId, Box<dyn Error>> {
    Ok(InvocationId::try_new(format!("inv-{sequence}"))?)
}
