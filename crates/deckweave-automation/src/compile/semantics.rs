// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    catalog::{CatalogEndpoint, CatalogSchema, DescriptorCatalog, SchemaValueKind},
    compiler::GraphCompileError,
};
use crate::{
    CompileDiagnostic, CompileDiagnosticCode, CompiledEndpointContract, CompiledSchemaContract,
    EndpointAvailabilityPolicy, GraphNode, GraphNodeKind, GraphPortRole, RuntimeRequirement,
};
use deckweave_domain::{automation::OperationDescriptor, capability::PortAvailability};
use deckweave_workspace::PackageLockEntry;
use std::collections::BTreeSet;

pub(super) fn validate_node_values(
    node: &GraphNode,
    catalog: &DescriptorCatalog,
) -> Result<(), GraphCompileError> {
    for port in node.inputs.iter().chain(&node.outputs) {
        let schema =
            catalog
                .schema(&port.schema)
                .ok_or_else(|| GraphCompileError::UnresolvedSchema {
                    node_id: node.id.clone(),
                })?;
        if port.unit != schema.unit || port.range != schema.range {
            return Err(GraphCompileError::SchemaConstraintMismatch {
                node_id: node.id.clone(),
            });
        }
    }
    match &node.kind {
        GraphNodeKind::State {
            initial_value: Some(value),
            ..
        } => {
            let schema = schema_for_first_output(node, catalog)?;
            if !schema.accepts(value) {
                return Err(GraphCompileError::InvalidValue {
                    node_id: node.id.clone(),
                });
            }
        }
        GraphNodeKind::Condition { operator, operand } => {
            let port =
                node.inputs
                    .first()
                    .ok_or_else(|| GraphCompileError::SchemaConstraintMismatch {
                        node_id: node.id.clone(),
                    })?;
            let schema = catalog.schema(&port.schema).ok_or_else(|| {
                GraphCompileError::UnresolvedSchema {
                    node_id: node.id.clone(),
                }
            })?;
            let valid = match operator {
                crate::ConditionOperator::IsTrue => {
                    schema.value_kind == SchemaValueKind::Boolean && operand.is_none()
                }
                crate::ConditionOperator::GreaterThan | crate::ConditionOperator::LessThan => {
                    schema.value_kind == SchemaValueKind::Number
                        && operand.as_ref().is_some_and(|value| schema.accepts(value))
                }
                crate::ConditionOperator::Equals | crate::ConditionOperator::NotEquals => {
                    operand.as_ref().is_some_and(|value| schema.accepts(value))
                }
            };
            if !valid {
                return Err(GraphCompileError::InvalidValue {
                    node_id: node.id.clone(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn validate_endpoint_ports(
    node: &GraphNode,
    endpoint: &CatalogEndpoint,
    catalog: &DescriptorCatalog,
) -> Result<(), GraphCompileError> {
    for port in node.inputs.iter().chain(&node.outputs) {
        let schema =
            catalog
                .schema(&port.schema)
                .ok_or_else(|| GraphCompileError::UnresolvedSchema {
                    node_id: node.id.clone(),
                })?;
        if endpoint.port.schema != port.schema
            || endpoint.port.unit != schema.unit
            || endpoint.port.range != schema.range
        {
            return Err(GraphCompileError::SchemaConstraintMismatch {
                node_id: node.id.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_action_ports(
    node: &GraphNode,
    operation: &OperationDescriptor,
    catalog: &DescriptorCatalog,
    arguments: Option<&crate::ActionArguments>,
) -> Result<(), GraphCompileError> {
    for port in node.inputs.iter().chain(&node.outputs) {
        let schema =
            catalog
                .schema(&port.schema)
                .ok_or_else(|| GraphCompileError::UnresolvedSchema {
                    node_id: node.id.clone(),
                })?;
        if port.unit != schema.unit || port.range != schema.range {
            return Err(GraphCompileError::SchemaConstraintMismatch {
                node_id: node.id.clone(),
            });
        }
        if arguments.is_some_and(|arguments| arguments.trigger_port == port.id) {
            continue;
        }
        let expected = match port.role {
            GraphPortRole::Trigger => continue,
            GraphPortRole::Input => &operation.input_schema,
            GraphPortRole::Result => &operation.result_schema,
            GraphPortRole::Error => &operation.error_schema,
            _ => {
                return Err(GraphCompileError::OperationMismatch {
                    node_id: node.id.clone(),
                });
            }
        };
        if port.schema != *expected {
            return Err(GraphCompileError::OperationMismatch {
                node_id: node.id.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn collect_availability(
    node: &GraphNode,
    endpoint: &CatalogEndpoint,
    policy: EndpointAvailabilityPolicy,
    diagnostics: &mut Vec<CompileDiagnostic>,
) {
    if policy == EndpointAvailabilityPolicy::Deferred
        && endpoint.port.availability.status != PortAvailability::Available
    {
        diagnostics.push(CompileDiagnostic {
            code: CompileDiagnosticCode::NeedsRuntimeAvailability,
            node_id: node.id.clone(),
            message: endpoint
                .port
                .availability
                .reason
                .clone()
                .unwrap_or_else(|| format!("{:?}", endpoint.port.availability.status)),
        });
    }
}

pub(super) fn collect_runtime_requirement(
    endpoint: &CatalogEndpoint,
    package_lock: &[PackageLockEntry],
    keys: &mut BTreeSet<(String, String, String, String, String, String)>,
    requirements: &mut Vec<RuntimeRequirement>,
) -> Result<(), GraphCompileError> {
    let requirement = &endpoint.runtime_requirement;
    if !package_lock.iter().any(|locked| {
        locked.package_id == requirement.package_id
            && locked.version == requirement.package_version
            && locked.artifact_sha256 == requirement.artifact_sha256
    }) {
        return Err(GraphCompileError::PackageLockMismatch {
            package_id: requirement.package_id.to_string(),
        });
    }
    let key = (
        requirement.package_id.to_string(),
        requirement.package_version.clone(),
        requirement.artifact_sha256.clone(),
        requirement.runtime_group_id.to_string(),
        requirement.contribution_id.to_string(),
        requirement
            .configuration
            .as_ref()
            .map(|value| value.instance_id.to_string())
            .unwrap_or_default(),
    );
    if keys.insert(key) {
        requirements.push(requirement.clone());
    }
    Ok(())
}

pub(super) fn contract(node: &GraphNode, endpoint: &CatalogEndpoint) -> CompiledEndpointContract {
    CompiledEndpointContract {
        configured_instance_id: endpoint
            .runtime_requirement
            .configuration
            .as_ref()
            .map(|c| c.instance_id.clone()),
        node_id: node.id.clone(),
        endpoint: endpoint.endpoint.clone(),
        runtime_group_id: endpoint.runtime_requirement.runtime_group_id.clone(),
        contribution_id: endpoint.runtime_requirement.contribution_id.clone(),
        port: endpoint.port.clone(),
        operation: endpoint.operation.clone(),
    }
}

pub(super) fn runtime_requirement_order(
    left: &RuntimeRequirement,
    right: &RuntimeRequirement,
) -> std::cmp::Ordering {
    left.package_id
        .cmp(&right.package_id)
        .then(left.runtime_group_id.cmp(&right.runtime_group_id))
        .then(left.contribution_id.cmp(&right.contribution_id))
        .then(
            left.configuration
                .as_ref()
                .map(|c| &c.instance_id)
                .cmp(&right.configuration.as_ref().map(|c| &c.instance_id)),
        )
}

fn schema_for_first_output<'a>(
    node: &GraphNode,
    catalog: &'a DescriptorCatalog,
) -> Result<&'a CatalogSchema, GraphCompileError> {
    let port = node
        .outputs
        .first()
        .ok_or_else(|| GraphCompileError::SchemaConstraintMismatch {
            node_id: node.id.clone(),
        })?;
    catalog
        .schema(&port.schema)
        .ok_or_else(|| GraphCompileError::UnresolvedSchema {
            node_id: node.id.clone(),
        })
}

pub(super) fn compile_schema_contracts(
    nodes: &[GraphNode],
    catalog: &DescriptorCatalog,
) -> Result<Vec<CompiledSchemaContract>, GraphCompileError> {
    let mut contracts = Vec::new();
    for node in nodes {
        let mut references: Vec<_> = node
            .inputs
            .iter()
            .chain(&node.outputs)
            .map(|port| port.schema.clone())
            .collect();
        if let GraphNodeKind::Action { endpoint, .. } = &node.kind
            && let Some(operation) = catalog
                .endpoint(endpoint)
                .and_then(|entry| entry.operation.as_ref())
        {
            references.extend([
                operation.input_schema.clone(),
                operation.result_schema.clone(),
                operation.error_schema.clone(),
            ]);
        }
        for reference in references {
            if contracts
                .iter()
                .any(|entry: &CompiledSchemaContract| entry.schema == reference)
            {
                continue;
            }
            let schema =
                catalog
                    .schema(&reference)
                    .ok_or_else(|| GraphCompileError::UnresolvedSchema {
                        node_id: node.id.clone(),
                    })?;
            contracts.push(CompiledSchemaContract {
                schema: schema.schema.clone(),
                value_kind: schema.effective_value_kind().ok_or_else(|| {
                    GraphCompileError::InvalidValue {
                        node_id: node.id.clone(),
                    }
                })?,
            });
        }
    }
    contracts.sort_by(|left, right| {
        left.schema
            .id
            .cmp(&right.schema.id)
            .then(left.schema.version.cmp(&right.schema.version))
    });
    Ok(contracts)
}
