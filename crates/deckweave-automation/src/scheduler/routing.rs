// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    PendingInvocation, SchedulerFault, SchedulerIntent, SchedulerState, SchedulerTransitionError,
};
use crate::{
    ConditionOperator, ExecutableGraph, GraphLimitKind, GraphLimits, GraphNodeKind, GraphPortId,
    GraphPortRole, SchemaValueKind,
};
use deckweave_domain::ids::{ExecutionId, GraphNodeId, InvocationId};
use deckweave_domain::schema::SchemaRef;
use serde_json::Value;
use std::collections::VecDeque;

struct Arrival {
    node_id: GraphNodeId,
    port_id: GraphPortId,
    value: Value,
}

pub(super) struct RouteContext<'a> {
    pub plan: &'a ExecutableGraph,
    pub state: &'a mut SchedulerState,
    pub run_id: &'a ExecutionId,
    pub now_ticks: u64,
    pub limits: &'a GraphLimits,
    pub intents: &'a mut Vec<SchedulerIntent>,
    pub faults: &'a mut Vec<SchedulerFault>,
}

pub(super) fn route_source(
    context: &mut RouteContext<'_>,
    source_node: &GraphNodeId,
    payload: &Value,
) -> Result<(), SchedulerTransitionError> {
    let node = node(context.plan, source_node)?;
    let mut arrivals = VecDeque::new();
    for port in &node.outputs {
        let value = match port.role {
            GraphPortRole::Trigger => Value::Null,
            GraphPortRole::Value => payload.clone(),
            _ => continue,
        };
        enqueue_edges(context.plan, source_node, &port.id, value, &mut arrivals);
    }
    route_arrivals(context, arrivals)
}

pub(super) fn route_state(
    context: &mut RouteContext<'_>,
    state_node: &GraphNodeId,
    value: Value,
) -> Result<(), SchedulerTransitionError> {
    context
        .state
        .values
        .insert(state_node.clone(), value.clone());
    route_role(context, state_node, GraphPortRole::Value, value)
}

pub(super) fn route_action_outcome(
    context: &mut RouteContext<'_>,
    action_node: &GraphNodeId,
    role: GraphPortRole,
    value: Value,
) -> Result<(), SchedulerTransitionError> {
    route_role(context, action_node, role, value)
}

pub(super) fn finish_run_if_idle(
    state: &mut SchedulerState,
    run_id: &ExecutionId,
    intents: &mut Vec<SchedulerIntent>,
) {
    if state
        .pending
        .values()
        .all(|pending| pending.run_id != *run_id)
        && state.active_runs.remove(run_id)
    {
        intents.push(SchedulerIntent::CleanupRun {
            run_id: run_id.clone(),
            reason: "run completed".into(),
        });
    }
}

fn route_role(
    context: &mut RouteContext<'_>,
    node_id: &GraphNodeId,
    role: GraphPortRole,
    value: Value,
) -> Result<(), SchedulerTransitionError> {
    let graph_node = node(context.plan, node_id)?;
    let mut arrivals = VecDeque::new();
    for port in graph_node.outputs.iter().filter(|port| port.role == role) {
        enqueue_edges(
            context.plan,
            node_id,
            &port.id,
            value.clone(),
            &mut arrivals,
        );
    }
    route_arrivals(context, arrivals)
}

fn route_arrivals(
    context: &mut RouteContext<'_>,
    mut arrivals: VecDeque<Arrival>,
) -> Result<(), SchedulerTransitionError> {
    while let Some(arrival) = arrivals.pop_front() {
        if !context.state.active_runs.contains(context.run_id) {
            return Ok(());
        }
        let graph_node = node(context.plan, &arrival.node_id)?;
        match &graph_node.kind {
            GraphNodeKind::Action { .. } => {
                let trigger = graph_node
                    .inputs
                    .iter()
                    .find(|port| port.id == arrival.port_id)
                    .is_some_and(|port| port.role == GraphPortRole::Trigger);
                let input = if trigger { Value::Null } else { arrival.value };
                invoke_action(context, graph_node, input)?;
            }
            GraphNodeKind::State { .. } => {
                context
                    .state
                    .values
                    .insert(graph_node.id.clone(), arrival.value.clone());
                enqueue_role(
                    context.plan,
                    graph_node,
                    GraphPortRole::Value,
                    arrival.value,
                    &mut arrivals,
                );
            }
            GraphNodeKind::Condition { operator, operand } => {
                let selected = if evaluate(*operator, operand.as_ref(), &arrival.value) {
                    GraphPortRole::True
                } else {
                    GraphPortRole::False
                };
                enqueue_role(
                    context.plan,
                    graph_node,
                    selected,
                    arrival.value,
                    &mut arrivals,
                );
            }
            GraphNodeKind::Feedback { endpoint, .. } => {
                context.intents.push(SchedulerIntent::Feedback {
                    run_id: context.run_id.clone(),
                    node_id: graph_node.id.clone(),
                    target: endpoint.clone(),
                    value: arrival.value,
                });
            }
            GraphNodeKind::Source { .. } => {
                return Err(SchedulerTransitionError::InvalidPlan(format!(
                    "source {} has an incoming edge at {}",
                    graph_node.id, arrival.port_id
                )));
            }
        }
    }
    Ok(())
}

fn invoke_action(
    context: &mut RouteContext<'_>,
    graph_node: &crate::GraphNode,
    input: Value,
) -> Result<(), SchedulerTransitionError> {
    let contract = context
        .plan
        .endpoint_contracts
        .iter()
        .find(|contract| contract.node_id == graph_node.id)
        .ok_or_else(|| {
            SchedulerTransitionError::InvalidPlan(format!(
                "action {} has no endpoint contract",
                graph_node.id
            ))
        })?;
    let operation = contract.operation.as_ref().ok_or_else(|| {
        SchedulerTransitionError::InvalidPlan(format!(
            "action {} has no operation contract",
            graph_node.id
        ))
    })?;
    let input_bytes = serialized_size(&input)?;
    if !value_matches_schema(context.plan, &operation.input_schema, &input)? {
        context.faults.push(SchedulerFault::MalformedInput);
        abort_run(context, "operation input does not match its schema");
        return Ok(());
    }
    if input_bytes > operation.limits.maximum_input_bytes as usize {
        context.faults.push(SchedulerFault::OversizedInput);
        abort_run(context, "operation input exceeds its byte limit");
        return Ok(());
    }
    if context.state.pending.len() >= context.limits.maximum_pending_invocations {
        context.faults.push(SchedulerFault::BudgetExceeded(
            GraphLimitKind::PendingInvocations,
        ));
        abort_run(context, "pending invocation budget exceeded");
        return Ok(());
    }
    if context.state.pending.len() >= context.limits.maximum_timers {
        context
            .faults
            .push(SchedulerFault::BudgetExceeded(GraphLimitKind::Timers));
        abort_run(context, "timer budget exceeded");
        return Ok(());
    }
    let pending_for_operation = context
        .state
        .pending
        .values()
        .filter(|pending| pending.node_id == graph_node.id)
        .count();
    if pending_for_operation >= operation.limits.maximum_concurrent_invocations as usize {
        context.faults.push(SchedulerFault::BudgetExceeded(
            GraphLimitKind::PendingInvocations,
        ));
        abort_run(context, "operation concurrency budget exceeded");
        return Ok(());
    }
    let sequence = context.state.next_invocation_sequence;
    context.state.next_invocation_sequence = sequence.checked_add(1).ok_or_else(|| {
        SchedulerTransitionError::InvalidIdentity("invocation sequence overflow".into())
    })?;
    let invocation_id = InvocationId::try_new(format!("inv-{sequence}"))
        .map_err(|error| SchedulerTransitionError::InvalidIdentity(error.to_string()))?;
    let deadline_ticks = context
        .now_ticks
        .checked_add(operation.limits.deadline_ms)
        .ok_or_else(|| SchedulerTransitionError::InvalidPlan("deadline overflow".into()))?;
    context.state.pending.insert(
        invocation_id.clone(),
        PendingInvocation {
            invocation_id: invocation_id.clone(),
            run_id: context.run_id.clone(),
            node_id: graph_node.id.clone(),
            deadline_ticks,
            accepted_job: None,
        },
    );
    context.intents.push(SchedulerIntent::Invoke {
        invocation_id,
        run_id: context.run_id.clone(),
        node_id: graph_node.id.clone(),
        target: contract.endpoint.clone(),
        input,
        deadline_ticks,
    });
    Ok(())
}

fn abort_run(context: &mut RouteContext<'_>, reason: &str) {
    let invocation_ids: Vec<_> = context
        .state
        .pending
        .values()
        .filter(|pending| pending.run_id == *context.run_id)
        .map(|pending| pending.invocation_id.clone())
        .collect();
    for invocation_id in invocation_ids {
        if let Some(pending) = context.state.pending.remove(&invocation_id) {
            context.intents.push(SchedulerIntent::CancelInvocation {
                invocation_id,
                job_id: pending.accepted_job,
                reason: reason.into(),
            });
        }
    }
    context.state.active_runs.remove(context.run_id);
    context.intents.push(SchedulerIntent::CleanupRun {
        run_id: context.run_id.clone(),
        reason: reason.into(),
    });
}

fn enqueue_role(
    plan: &ExecutableGraph,
    graph_node: &crate::GraphNode,
    role: GraphPortRole,
    value: Value,
    arrivals: &mut VecDeque<Arrival>,
) {
    for port in graph_node.outputs.iter().filter(|port| port.role == role) {
        enqueue_edges(plan, &graph_node.id, &port.id, value.clone(), arrivals);
    }
}

fn enqueue_edges(
    plan: &ExecutableGraph,
    node_id: &GraphNodeId,
    port_id: &GraphPortId,
    value: Value,
    arrivals: &mut VecDeque<Arrival>,
) {
    for edge in plan
        .edges
        .iter()
        .filter(|edge| edge.source.node_id == *node_id && edge.source.port_id == *port_id)
    {
        arrivals.push_back(Arrival {
            node_id: edge.target.node_id.clone(),
            port_id: edge.target.port_id.clone(),
            value: value.clone(),
        });
    }
}

fn node<'a>(
    plan: &'a ExecutableGraph,
    node_id: &GraphNodeId,
) -> Result<&'a crate::GraphNode, SchedulerTransitionError> {
    plan.nodes
        .iter()
        .find(|node| node.id == *node_id)
        .ok_or_else(|| SchedulerTransitionError::InvalidPlan(format!("missing node {node_id}")))
}

fn evaluate(operator: ConditionOperator, operand: Option<&Value>, value: &Value) -> bool {
    match operator {
        ConditionOperator::IsTrue => value.as_bool().unwrap_or(false),
        ConditionOperator::Equals => operand == Some(value),
        ConditionOperator::NotEquals => operand != Some(value),
        ConditionOperator::GreaterThan => numeric(value)
            .zip(operand.and_then(numeric))
            .is_some_and(|(left, right)| left > right),
        ConditionOperator::LessThan => numeric(value)
            .zip(operand.and_then(numeric))
            .is_some_and(|(left, right)| left < right),
    }
}

fn numeric(value: &Value) -> Option<f64> {
    value.as_f64()
}

pub(super) fn value_matches_schema(
    plan: &ExecutableGraph,
    schema: &SchemaRef,
    value: &Value,
) -> Result<bool, SchedulerTransitionError> {
    let contract = plan
        .schema_contracts
        .iter()
        .find(|contract| contract.schema == *schema)
        .ok_or_else(|| {
            SchedulerTransitionError::InvalidPlan(format!(
                "missing schema contract {} {}.{}",
                schema.id, schema.version.major, schema.version.minor
            ))
        })?;
    Ok(match contract.value_kind {
        SchemaValueKind::Unit => value.is_null(),
        SchemaValueKind::Boolean => value.is_boolean(),
        SchemaValueKind::Number => value.is_number(),
        SchemaValueKind::String => value.is_string(),
        SchemaValueKind::Json => true,
    })
}

pub(super) fn node_value_matches(
    plan: &ExecutableGraph,
    node_id: &GraphNodeId,
    role: GraphPortRole,
    value: &Value,
) -> Result<bool, SchedulerTransitionError> {
    let graph_node = node(plan, node_id)?;
    let mut matched = false;
    for port in graph_node.outputs.iter().filter(|port| port.role == role) {
        matched = true;
        if !value_matches_schema(plan, &port.schema, value)? {
            return Ok(false);
        }
    }
    Ok(matched)
}

fn serialized_size(value: &Value) -> Result<usize, SchedulerTransitionError> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| {
            SchedulerTransitionError::InvalidPlan(format!("value serialization failed: {error}"))
        })
}
