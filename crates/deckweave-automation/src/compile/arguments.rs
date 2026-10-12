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
    let SchemaValueKind::Structured(PayloadSchema::Record { fields }) = &schema.value_kind else {
        return Err(invalid());
    };
    if fields
        .iter()
        .any(|(name, field)| !field.optional && !binding.fields.contains_key(name))
    {
        return Err(invalid());
    }
    for (name, value) in &binding.fields {
        let expected = &fields.get(name).ok_or_else(invalid)?.schema;
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
        // Nested physical quantities must use explicit records/converters. Never discard units.
        if source.unit.is_some() {
            return Err(invalid());
        }
        let mut shape = match &source.value_kind {
            SchemaValueKind::Structured(shape) => shape.clone(),
            SchemaValueKind::Unit => PayloadSchema::Null,
            SchemaValueKind::Boolean => PayloadSchema::Boolean,
            SchemaValueKind::String => PayloadSchema::String {
                maximum_bytes: 1024 * 1024,
                choices: vec![],
            },
            SchemaValueKind::Number => PayloadSchema::Number {
                minimum: source.range.as_ref().map_or(f64::MIN, |r| r.minimum),
                maximum: source.range.as_ref().map_or(f64::MAX, |r| r.maximum),
            },
            SchemaValueKind::Json => return Err(invalid()),
        };
        // Flow edges carry a scheduling token; their declared schema does not
        // turn that token into the source's payload. Read that via RunResult.
        if matches!(value, ValueBinding::Trigger { .. })
            && node
                .inputs
                .iter()
                .any(|port| port.id == binding.trigger_port && port.kind == GraphPortKind::Flow)
        {
            shape = PayloadSchema::Null;
        }
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
                        if field
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
