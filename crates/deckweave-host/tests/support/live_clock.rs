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
    let _pump = ExecutionTimerPump::start(&owner, Duration::from_millis(2));
    clock.0.store(600_101, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !owner.snapshot()?.ok_or("scheduler")?.pending.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(owner.snapshot()?.ok_or("scheduler")?.pending.is_empty());
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
