// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    IngressItem, IngressKind, SchedulerFault, SchedulerIntent, SchedulerState, SchedulerTransition,
    SchedulerTransitionError,
    invocation::{process_deadline, process_invocation},
    routing::{RouteContext, finish_run_if_idle, node_value_matches, route_source, route_state},
};
use crate::{ExecutableGraph, GraphLimitKind, GraphLimits, GraphPortRole};
use deckweave_domain::ids::ExecutionId;
use serde_json::Value;

pub fn transition(
    plan: &ExecutableGraph,
    mut state: SchedulerState,
    item: IngressItem,
    now_ticks: u64,
    limits: &GraphLimits,
) -> Result<SchedulerTransition, SchedulerTransitionError> {
    let mut intents = Vec::new();
    let mut faults = Vec::new();
    if item.context.host_boot_id != state.host_boot_id
        || item.context.activation_epoch != state.activation_epoch
    {
        faults.push(SchedulerFault::StaleIngress);
        return Ok(SchedulerTransition {
            state,
            intents,
            faults,
        });
    }
    if let Some(previous) = state.last_ordinal
        && item.context.ordinal <= previous
    {
        return Err(SchedulerTransitionError::IngressOutOfOrder {
            previous: previous.get(),
            actual: item.context.ordinal.get(),
        });
    }
    state.last_ordinal = Some(item.context.ordinal);
    for slot in &plan.state_slots {
        if let Some(initial_value) = &slot.initial_value {
            state
                .values
                .entry(slot.node_id.clone())
                .or_insert_with(|| initial_value.clone());
        }
    }
    if state.closed && !matches!(item.kind, IngressKind::Barrier) {
        faults.push(SchedulerFault::BarrierClosed);
        return Ok(SchedulerTransition {
            state,
            intents,
            faults,
        });
    }

    match item.kind {
        IngressKind::Signal {
            source,
            payload,
            source_ticks,
            ..
        } => {
            state.source_ticks.insert(source.clone(), source_ticks);
            if !source_value_matches(plan, &source, &payload)? {
                faults.push(SchedulerFault::MalformedInput);
            } else if serialized_size(&payload)? > limits.maximum_payload_bytes {
                faults.push(SchedulerFault::BudgetExceeded(GraphLimitKind::PayloadBytes));
            } else {
                let sources: Vec<_> = plan
                    .source_subscriptions
                    .iter()
                    .filter(|subscription| subscription.endpoint == source)
                    .map(|subscription| subscription.node_id.clone())
                    .collect();
                if sources.is_empty() {
                    faults.push(SchedulerFault::UnknownSource);
                } else if let Some(run_id) = start_run(&mut state, limits, &mut faults)? {
                    for source_node in sources {
                        if !state.active_runs.contains(&run_id) {
                            break;
                        }
                        let mut context = RouteContext {
                            plan,
                            state: &mut state,
                            run_id: &run_id,
                            now_ticks,
                            limits,
                            intents: &mut intents,
                            faults: &mut faults,
                        };
                        route_source(&mut context, &source_node, &payload)?;
                    }
                    finish_run_if_idle(&mut state, &run_id, &mut intents);
                }
            }
        }
        IngressKind::StateUpdate { node_id, value } => {
            if !plan.state_slots.iter().any(|slot| slot.node_id == node_id) {
                faults.push(SchedulerFault::UnknownState);
            } else if !node_value_matches(plan, &node_id, GraphPortRole::Value, &value)? {
                faults.push(SchedulerFault::MalformedInput);
            } else if serialized_size(&value)? > limits.maximum_payload_bytes {
                faults.push(SchedulerFault::BudgetExceeded(GraphLimitKind::PayloadBytes));
            } else if let Some(run_id) = start_run(&mut state, limits, &mut faults)? {
                let mut context = RouteContext {
                    plan,
                    state: &mut state,
                    run_id: &run_id,
                    now_ticks,
                    limits,
                    intents: &mut intents,
                    faults: &mut faults,
                };
                route_state(&mut context, &node_id, value)?;
                finish_run_if_idle(&mut state, &run_id, &mut intents);
            }
        }
        IngressKind::TimerDeadline { invocation_id } => process_deadline(
            &mut state,
            invocation_id,
            now_ticks,
            &mut intents,
            &mut faults,
        ),
        IngressKind::Invocation { reply } => process_invocation(
            plan,
            &mut state,
            reply,
            now_ticks,
            limits,
            &mut intents,
            &mut faults,
        )?,
        IngressKind::TopologyFault { .. } => {
            faults.push(SchedulerFault::TopologyChanged);
            cleanup_all(&mut state, "runtime topology changed", &mut intents);
        }
        IngressKind::GrantFault => {
            faults.push(SchedulerFault::GrantRevoked);
            cleanup_all(&mut state, "runtime grant revoked", &mut intents);
        }
        IngressKind::Barrier => {
            state.closed = true;
            cleanup_all(&mut state, "activation barrier", &mut intents);
        }
    }
    Ok(SchedulerTransition {
        state,
        intents,
        faults,
    })
}

fn start_run(
    state: &mut SchedulerState,
    limits: &GraphLimits,
    faults: &mut Vec<SchedulerFault>,
) -> Result<Option<ExecutionId>, SchedulerTransitionError> {
    if state.active_runs.len() >= limits.maximum_pending_runs {
        faults.push(SchedulerFault::BudgetExceeded(GraphLimitKind::PendingRuns));
        return Ok(None);
    }
    let sequence = state.next_run_sequence;
    state.next_run_sequence = sequence
        .checked_add(1)
        .ok_or_else(|| SchedulerTransitionError::InvalidIdentity("run sequence overflow".into()))?;
    let run_id = ExecutionId::try_new(format!("run-{sequence}"))
        .map_err(|error| SchedulerTransitionError::InvalidIdentity(error.to_string()))?;
    state.active_runs.insert(run_id.clone());
    Ok(Some(run_id))
}

fn cleanup_all(state: &mut SchedulerState, reason: &str, intents: &mut Vec<SchedulerIntent>) {
    for (_, pending) in std::mem::take(&mut state.pending) {
        intents.push(SchedulerIntent::CancelInvocation {
            invocation_id: pending.invocation_id,
            job_id: pending.accepted_job,
            reason: reason.into(),
        });
    }
    state.active_runs.clear();
    state.run_results.clear();
    intents.push(SchedulerIntent::CleanupAll {
        reason: reason.into(),
    });
}

fn serialized_size<T: serde::Serialize>(value: &T) -> Result<usize, SchedulerTransitionError> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| SchedulerTransitionError::InvalidPlan(error.to_string()))
}

fn source_value_matches(
    plan: &ExecutableGraph,
    source: &deckweave_domain::endpoint::StableEndpointRef,
    value: &Value,
) -> Result<bool, SchedulerTransitionError> {
    let Some(subscription) = plan
        .source_subscriptions
        .iter()
        .find(|subscription| subscription.endpoint == *source)
    else {
        return Ok(true);
    };
    let node = plan
        .nodes
        .iter()
        .find(|node| node.id == subscription.node_id)
        .ok_or_else(|| SchedulerTransitionError::InvalidPlan("source node is missing".into()))?;
    for port in &node.outputs {
        if !super::routing::value_matches_schema(plan, &port.schema, value)? {
            return Ok(false);
        }
    }
    Ok(true)
}
