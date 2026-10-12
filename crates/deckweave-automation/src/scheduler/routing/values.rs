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
