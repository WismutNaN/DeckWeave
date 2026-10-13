// SPDX-License-Identifier: GPL-3.0-or-later
use super::{CatalogEndpoint, DescriptorCatalog, GraphCompileError, SchemaValueKind};
use crate::{ActionArguments, GraphNode, GraphNodeKind, GraphPortKind, ValueBinding};
use deckweave_domain::schema::PayloadSchema;

pub(super) fn validate(
    action: &GraphNode,
    binding: &ActionArguments,
    nodes: &[GraphNode],
    entry: &CatalogEndpoint,
    catalog: &DescriptorCatalog,
) -> Result<(), GraphCompileError> {
    let invalid = || GraphCompileError::InvalidValue {
        node_id: action.id.clone(),
    };
    let operation = entry.operation.as_ref().ok_or_else(invalid)?;
    let schema = catalog
        .schema(&operation.input_schema)
        .ok_or_else(invalid)?;
    let root_shape;
    let arguments: Vec<_> = if let Some(value) = &binding.value {
        root_shape = payload_shape(schema).ok_or_else(invalid)?;
        vec![("$input", value, &root_shape, schema.unit.as_deref())]
    } else {
        let SchemaValueKind::Structured(PayloadSchema::Record { fields }) = &schema.value_kind
        else {
            return Err(invalid());
        };
        // A record assembled from fields has no unit-bearing wrapper.
        if schema.unit.is_some() || schema.range.is_some() {
            return Err(invalid());
        }
        if let Some((name, _)) = fields
            .iter()
            .find(|(name, field)| !field.optional && !binding.fields.contains_key(*name))
        {
            return Err(GraphCompileError::InvalidArgument {
                node_id: action.id.clone(),
                field: name.clone(),
            });
        }
        binding
            .fields
            .iter()
            .map(|(name, value)| {
                fields
                    .get(name)
                    .map(|field| (name.as_str(), value, &field.schema, None))
                    .ok_or_else(|| GraphCompileError::InvalidArgument {
                        node_id: action.id.clone(),
                        field: name.clone(),
                    })
            })
            .collect::<Result<_, _>>()?
    };
    for (name, value, expected, target_unit) in arguments {
        let invalid = || GraphCompileError::InvalidArgument {
            node_id: action.id.clone(),
            field: name.to_owned(),
        };
        let (node, pointer) = match value {
            ValueBinding::Constant { value } => {
                if !expected.accepts(value) {
                    return Err(invalid());
                }
                continue;
            }
            ValueBinding::Trigger { pointer } => (action, pointer),
            ValueBinding::State { node_id, pointer }
            | ValueBinding::RunResult { node_id, pointer } => (
                nodes
                    .iter()
                    .find(|n| &n.id == node_id)
                    .ok_or_else(invalid)?,
                pointer,
            ),
        };
        let reference = if matches!(value, ValueBinding::Trigger { .. }) {
            &node
                .inputs
                .iter()
                .find(|p| p.id == binding.trigger_port)
                .ok_or_else(invalid)?
                .schema
        } else if let GraphNodeKind::Action { endpoint, .. } = &node.kind {
            &catalog
                .endpoint(endpoint)
                .and_then(|e| e.operation.as_ref())
                .ok_or_else(invalid)?
                .result_schema
        } else {
            &node.outputs.first().ok_or_else(invalid)?.schema
        };
        let source = catalog.schema(reference).ok_or_else(invalid)?;
        let flow_trigger = matches!(value, ValueBinding::Trigger { .. })
            && node
                .inputs
                .iter()
                .any(|port| port.id == binding.trigger_port && port.kind == GraphPortKind::Flow);
        // Flow is only a scheduling token. Whole values preserve their unit;
        // pointers into unit-bearing values need an explicit converter.
        let source_unit = if flow_trigger {
            None
        } else {
            source.unit.as_deref()
        };
        if source_unit != target_unit || (source_unit.is_some() && !pointer.is_empty()) {
            return Err(invalid());
        }
        let mut shape = if flow_trigger {
            PayloadSchema::Null
        } else {
            payload_shape(source).ok_or_else(invalid)?
        };
        if !pointer.is_empty() {
            for segment in pointer.strip_prefix('/').ok_or_else(invalid)?.split('/') {
                let field = segment.replace("~1", "/").replace("~0", "~");
                shape = match shape {
                    PayloadSchema::Record { mut fields } => {
                        fields.remove(&field).ok_or_else(invalid)?.schema
                    }
                    PayloadSchema::Array {
                        items,
                        maximum_items,
                    } => {
                        if (field.len() > 1 && field.starts_with('0'))
                            || !field.bytes().all(|b| b.is_ascii_digit())
                            || field
                                .parse::<usize>()
                                .ok()
                                .is_none_or(|i| i >= maximum_items)
                        {
                            return Err(invalid());
                        }
                        *items
                    }
                    _ => return Err(invalid()),
                };
            }
        }
        if !assignable(&shape, expected) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn assignable(source: &PayloadSchema, target: &PayloadSchema) -> bool {
    use PayloadSchema::*;
    match (source, target) {
        (
            String {
                maximum_bytes: s,
                choices: sc,
            },
            String {
                maximum_bytes: t,
                choices: tc,
            },
        ) => s <= t && (tc.is_empty() || (!sc.is_empty() && sc.iter().all(|v| tc.contains(v)))),
        (
            Number {
                minimum: s0,
                maximum: s1,
            },
            Number {
                minimum: t0,
                maximum: t1,
            },
        ) => s0 >= t0 && s1 <= t1,
        _ => source == target,
    }
}

fn payload_shape(schema: &super::CatalogSchema) -> Option<PayloadSchema> {
    let mut shape = match &schema.value_kind {
        SchemaValueKind::Structured(shape) => shape.clone(),
        SchemaValueKind::Unit => PayloadSchema::Null,
        SchemaValueKind::Boolean => PayloadSchema::Boolean,
        SchemaValueKind::String => PayloadSchema::String {
            maximum_bytes: 1024 * 1024,
            choices: vec![],
        },
        SchemaValueKind::Number => PayloadSchema::Number {
            minimum: f64::MIN,
            maximum: f64::MAX,
        },
        SchemaValueKind::Json => return None,
    };
    if let Some(range) = &schema.range {
        let PayloadSchema::Number { minimum, maximum } = &mut shape else {
            return None;
        };
        *minimum = minimum.max(range.minimum);
        *maximum = maximum.min(range.maximum);
        if *minimum > *maximum {
            return None;
        }
    }
    Some(shape)
}
