// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
pub(in crate::scheduler) fn numeric(value: &Value) -> Option<f64> {
    value.as_f64()
}

pub(in crate::scheduler) fn value_matches_schema(
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
    Ok(contract.value_kind.accepts(value))
}

pub(in crate::scheduler) fn node_value_matches(
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

pub(in crate::scheduler) fn serialized_size(
    value: &Value,
) -> Result<usize, SchedulerTransitionError> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| {
            SchedulerTransitionError::InvalidPlan(format!("value serialization failed: {error}"))
        })
}

pub(super) fn remember_result(
    context: &mut RouteContext<'_>,
    node_id: &GraphNodeId,
    value: &Value,
) -> Result<bool, SchedulerTransitionError> {
    let referenced = context.plan.action_arguments.values().any(|arguments| arguments.bindings().any(|binding| matches!(binding, crate::ValueBinding::RunResult { node_id: source, .. } if source == node_id)));
    if !referenced {
        return Ok(true);
    }
    let results = context
        .state
        .run_results
        .entry(context.run_id.clone())
        .or_default();
    results.insert(node_id.clone(), value.clone());
    let size = serde_json::to_vec(results)
        .map_err(|e| SchedulerTransitionError::InvalidPlan(e.to_string()))?
        .len();
    if size > context.limits.maximum_payload_bytes {
        context
            .faults
            .push(SchedulerFault::BudgetExceeded(GraphLimitKind::PayloadBytes));
        abort_run(context, "retained run values exceed budget");
        return Ok(false);
    }
    Ok(true)
}
