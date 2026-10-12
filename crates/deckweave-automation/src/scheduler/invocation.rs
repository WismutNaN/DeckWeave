// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    PendingInvocation, SchedulerFault, SchedulerIntent, SchedulerState, SchedulerTransitionError,
    routing::{RouteContext, finish_run_if_idle, node_value_matches, route_action_outcome},
};
use crate::{ExecutableGraph, GraphLimits, GraphPortRole};
use deckweave_domain::{
    automation::{CommandOutcome, DeckWeaveError, InvocationReply},
    ids::InvocationId,
};
use serde_json::Value;

#[allow(clippy::too_many_arguments)]
pub(super) fn process_invocation(
    plan: &ExecutableGraph,
    state: &mut SchedulerState,
    reply: InvocationReply,
    now_ticks: u64,
    limits: &GraphLimits,
    intents: &mut Vec<SchedulerIntent>,
    faults: &mut Vec<SchedulerFault>,
) -> Result<(), SchedulerTransitionError> {
    match reply {
        InvocationReply::AcceptedJob {
            invocation_id,
            job_id,
        } => {
            let Some(pending) = state.pending.get_mut(&invocation_id) else {
                faults.push(SchedulerFault::UnknownInvocation);
                return Ok(());
            };
            match &pending.accepted_job {
                None => pending.accepted_job = Some(job_id),
                Some(existing) if *existing == job_id => {}
                Some(_) => faults.push(SchedulerFault::ConflictingAcceptance),
            }
            Ok(())
        }
        InvocationReply::Terminal {
            invocation_id,
            outcome,
        } => terminal(
            plan,
            state,
            invocation_id,
            outcome,
            now_ticks,
            limits,
            intents,
            faults,
        ),
        InvocationReply::Rejected {
            invocation_id,
            error,
        } => terminal(
            plan,
            state,
            invocation_id,
            CommandOutcome::Failed { error },
            now_ticks,
            limits,
            intents,
            faults,
        ),
    }
}

pub(super) fn process_deadline(
    state: &mut SchedulerState,
    invocation_id: InvocationId,
    now_ticks: u64,
    intents: &mut Vec<SchedulerIntent>,
    faults: &mut Vec<SchedulerFault>,
) {
    let Some(pending) = state.pending.get(&invocation_id) else {
        faults.push(SchedulerFault::UnknownInvocation);
        return;
    };
    if now_ticks < pending.deadline_ticks {
        return;
    }
    let Some(pending) = state.pending.remove(&invocation_id) else {
        faults.push(SchedulerFault::UnknownInvocation);
        return;
    };
    faults.push(SchedulerFault::DeadlineExceeded);
    intents.push(SchedulerIntent::CancelInvocation {
        invocation_id,
        job_id: pending.accepted_job,
        reason: "operation deadline exceeded".into(),
    });
    finish_run_if_idle(state, &pending.run_id, intents);
}

#[allow(clippy::too_many_arguments)]
fn terminal(
    plan: &ExecutableGraph,
    state: &mut SchedulerState,
    invocation_id: InvocationId,
    outcome: CommandOutcome,
    now_ticks: u64,
    limits: &GraphLimits,
    intents: &mut Vec<SchedulerIntent>,
    faults: &mut Vec<SchedulerFault>,
) -> Result<(), SchedulerTransitionError> {
    if let Some(existing) = state.completed.get(&invocation_id) {
        faults.push(if *existing == outcome {
            SchedulerFault::DuplicateTerminal
        } else {
            SchedulerFault::ConflictingTerminal
        });
        return Ok(());
    }
    if state
        .pending
        .get(&invocation_id)
        .is_some_and(|pending| now_ticks >= pending.deadline_ticks)
    {
        process_deadline(state, invocation_id, now_ticks, intents, faults);
        return Ok(());
    }
    let Some(pending) = state.pending.remove(&invocation_id) else {
        faults.push(SchedulerFault::UnknownInvocation);
        return Ok(());
    };
    let result_limit = operation_output_limit(plan, &pending)?;
    let (role, value) = outcome_route(&outcome)?;
    let malformed =
        outcome.validate().is_err() || !node_value_matches(plan, &pending.node_id, role, &value)?;
    let oversized = serialized_size(&value)? > result_limit;
    remember_completion(state, invocation_id, outcome.clone(), limits);
    if malformed {
        faults.push(SchedulerFault::MalformedResult);
    } else if oversized {
        faults.push(SchedulerFault::OversizedResult);
    } else {
        let run_id = pending.run_id.clone();
        let mut context = RouteContext {
            plan,
            state,
            run_id: &run_id,
            now_ticks,
            limits,
            intents,
            faults,
        };
        route_action_outcome(&mut context, &pending.node_id, role, value)?;
    }
    finish_run_if_idle(state, &pending.run_id, intents);
    Ok(())
}

fn operation_output_limit(
    plan: &ExecutableGraph,
    pending: &PendingInvocation,
) -> Result<usize, SchedulerTransitionError> {
    let operation = plan
        .endpoint_contracts
        .iter()
        .find(|contract| contract.node_id == pending.node_id)
        .and_then(|contract| contract.operation.as_ref())
        .ok_or_else(|| {
            SchedulerTransitionError::InvalidPlan(format!(
                "pending action {} has no operation contract",
                pending.node_id
            ))
        })?;
    usize::try_from(operation.limits.maximum_output_bytes).map_err(|_| {
        SchedulerTransitionError::InvalidPlan("operation output limit exceeds usize".into())
    })
}

fn remember_completion(
    state: &mut SchedulerState,
    invocation_id: InvocationId,
    outcome: CommandOutcome,
    limits: &GraphLimits,
) {
    while state.completed.len() >= limits.maximum_trace_entries {
        let Some(evicted) = state.completion_order.pop_front() else {
            break;
        };
        state.completed.remove(&evicted);
    }
    state.completion_order.push_back(invocation_id.clone());
    state.completed.insert(invocation_id, outcome);
}

fn outcome_route(
    outcome: &CommandOutcome,
) -> Result<(GraphPortRole, Value), SchedulerTransitionError> {
    match outcome {
        CommandOutcome::Succeeded { result } => Ok((GraphPortRole::Result, result.clone())),
        CommandOutcome::Failed { error }
        | CommandOutcome::Cancelled { error, .. }
        | CommandOutcome::UnknownOutcome { error, .. } => {
            Ok((GraphPortRole::Error, error_value(error)?))
        }
    }
}

fn error_value(error: &DeckWeaveError) -> Result<Value, SchedulerTransitionError> {
    serde_json::to_value(error).map_err(|error| {
        SchedulerTransitionError::InvalidPlan(format!("error serialization failed: {error}"))
    })
}

fn serialized_size<T: serde::Serialize>(value: &T) -> Result<usize, SchedulerTransitionError> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| SchedulerTransitionError::InvalidPlan(error.to_string()))
}
