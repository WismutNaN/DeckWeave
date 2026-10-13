// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_host::{ExecutionClock, ExecutionTimerPump};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Clock(AtomicU64);
impl ExecutionClock for Clock {
    fn now_ticks(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct FailedReleases(AtomicU64);
impl RuntimeIntentDispatcher for FailedReleases {
    fn dispatch(&self, command: RuntimeCommand) -> Result<(), RuntimeDispatchFailure> {
        if matches!(
            command,
            RuntimeCommand::Cancel { .. } | RuntimeCommand::CleanupRun { .. }
        ) {
            self.0.fetch_add(1, Ordering::SeqCst);
            return Err(RuntimeDispatchFailure {
                error: test_error(),
                possibly_sent: false,
            });
        }
        Ok(())
    }
}

#[test]
fn failed_release_does_not_starve_other_or_future_deadlines() -> Result<(), Box<dyn Error>> {
    let clock = Arc::new(Clock(AtomicU64::new(0)));
    let dispatcher = Arc::new(FailedReleases::default());
    let owner = Arc::new(LiveExecutionOwner::with_clock(
        HostBootId::try_new("failed-release-clock")?,
        dispatcher.clone(),
        clock.clone(),
    ));
    let (mut graph, binding, source) = fixture()?;
    graph.nodes[1].kind = GraphNodeKind::Action {
        endpoint: graph.endpoint_contracts[1].endpoint.clone(),
        operation_id: OperationId::try_new("uncertain")?,
        availability: EndpointAvailabilityPolicy::Required,
    };
    graph.nodes[1].inputs[0].role = GraphPortRole::Input;
    graph.nodes[1].outputs = vec![graph_port("result", GraphPortRole::Result, schema()?)?];
    let mut operation = operation(schema()?, schema()?, schema()?)?;
    operation.limits.deadline_ms = 100;
    operation.limits.maximum_concurrent_invocations = 2;
    graph.endpoint_contracts[1].operation = Some(operation);
    let epoch = owner.activate("plan", Some(&graph), Some(&binding), GraphLimits::default())?;
    owner.publish(signal(epoch, source.clone(), true));
    owner.publish(signal(epoch, source.clone(), true));
    assert_eq!(owner.snapshot()?.ok_or("scheduler")?.pending.len(), 2);
    clock.0.store(101, Ordering::SeqCst);
    assert!(matches!(
        owner.poll_deadlines(),
        Err(deckweave_host::LiveExecutionError::Cleanup(_))
    ));
    assert!(owner.snapshot()?.ok_or("scheduler")?.pending.is_empty());
    let _pump = ExecutionTimerPump::start(&owner, Duration::from_millis(2));
    for now in [202, 303] {
        owner.publish(signal(epoch, source.clone(), true));
        assert_eq!(owner.snapshot()?.ok_or("scheduler")?.pending.len(), 1);
        clock.0.store(now, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !owner.snapshot()?.ok_or("scheduler")?.pending.is_empty() && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(owner.snapshot()?.ok_or("scheduler")?.pending.is_empty());
    }
    assert!(dispatcher.0.load(Ordering::SeqCst) >= 4);
    Ok(())
}

#[test]
fn idle_live_job_expires_after_long_host_uptime_without_input_events() -> Result<(), Box<dyn Error>>
{
    let clock = Arc::new(Clock(AtomicU64::new(600_000)));
    let dispatcher = Arc::new(RecordingDispatcher::default());
    let owner = Arc::new(LiveExecutionOwner::with_clock(
        HostBootId::try_new("clock")?,
        dispatcher.clone(),
        clock.clone(),
    ));
    let (mut graph, binding, source) = fixture()?;
    graph.nodes[1].kind = GraphNodeKind::Action {
        endpoint: graph.endpoint_contracts[1].endpoint.clone(),
        operation_id: OperationId::try_new("uncertain")?,
        availability: EndpointAvailabilityPolicy::Required,
    };
    graph.nodes[1].inputs[0].role = GraphPortRole::Input;
    graph.nodes[1].outputs = vec![graph_port("result", GraphPortRole::Result, schema()?)?];
    let mut operation = operation(schema()?, schema()?, schema()?)?;
    operation.limits.deadline_ms = 100;
    graph.endpoint_contracts[1].operation = Some(operation);
    let epoch = owner.activate(
        "clock-plan",
        Some(&graph),
        Some(&binding),
        GraphLimits::default(),
    )?;
    owner.publish(signal(epoch, source, true));
    assert_eq!(
        owner
            .snapshot()?
            .ok_or("scheduler")?
            .pending
            .values()
            .next()
            .ok_or("pending")?
            .deadline_ticks,
        600_100
    );
    let before = owner.snapshot()?.ok_or("scheduler")?;
    assert!(owner.can_retain_execution(Some(&graph), Some(&binding))?);
    owner.retain_execution("layout-revision", Some(&graph), Some(&binding))?;
    let after = owner.snapshot()?.ok_or("scheduler")?;
    assert_eq!(after.activation_epoch, epoch);
    assert_eq!(after.pending, before.pending);
    assert_eq!(after.active_runs, before.active_runs);
    assert_eq!(
        after.next_invocation_sequence,
        before.next_invocation_sequence
    );
    let mut changed = graph.clone();
    changed.endpoint_contracts[1]
        .operation
        .as_mut()
        .ok_or("operation")?
        .limits
        .deadline_ms = 200;
    assert!(!owner.can_retain_execution(Some(&changed), Some(&binding))?);
    assert!(
        owner
            .retain_execution("changed", Some(&changed), Some(&binding))
            .is_err()
    );
    let _pump = ExecutionTimerPump::start(&owner, Duration::from_millis(2));
    clock.0.store(600_101, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !owner.snapshot()?.ok_or("scheduler")?.pending.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(owner.snapshot()?.ok_or("scheduler")?.pending.is_empty());
    owner.close_barrier(600_102)?;
    assert!(!owner.can_retain_execution(Some(&graph), Some(&binding))?);
    assert!(
        owner
            .retain_execution("closed", Some(&graph), Some(&binding))
            .is_err()
    );
    assert!(
        dispatcher
            .0
            .lock()
            .map_err(|_| "lock")?
            .iter()
            .any(|command| matches!(command, RuntimeCommand::Cancel { .. }))
    );
    Ok(())
}
