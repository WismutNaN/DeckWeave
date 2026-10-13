// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
pub(super) fn validate(
    document: &GraphDocument,
    nodes: &BTreeMap<GraphNodeId, &GraphNode>,
    limits: &GraphLimits,
) -> Result<(), GraphValidationError> {
    for (node_id, arguments) in &document.graph.action_arguments {
        let invalid = || GraphValidationError::InvalidNodeShape {
            node_id: node_id.clone(),
            node_kind: "action arguments",
        };
        let node = nodes.get(node_id).ok_or_else(invalid)?;
        if !matches!(node.kind, GraphNodeKind::Action { .. })
            || (arguments.value.is_some()
                && (document.schema_version.minor < 1 || !arguments.fields.is_empty()))
            || arguments.fields.len() > limits.maximum_ports_per_node
            || !node
                .inputs
                .iter()
                .any(|port| port.id == arguments.trigger_port)
            || document
                .graph
                .edges
                .iter()
                .filter(|edge| {
                    edge.target.node_id == *node_id && edge.target.port_id == arguments.trigger_port
                })
                .count()
                != 1
        {
            return Err(invalid());
        }
        if arguments
            .fields
            .keys()
            .any(|name| name.is_empty() || name.len() > 128)
        {
            return Err(invalid());
        }
        for binding in arguments.bindings() {
            use crate::ValueBinding;
            let pointer = match binding {
                ValueBinding::Constant { value } => {
                    if !deckweave_domain::schema::bounded_json(value) {
                        return Err(invalid());
                    }
                    continue;
                }
                ValueBinding::Trigger { pointer } => pointer,
                ValueBinding::State {
                    node_id: source,
                    pointer,
                } => {
                    if !nodes
                        .get(source)
                        .is_some_and(|n| matches!(n.kind, GraphNodeKind::State { .. }))
                    {
                        return Err(invalid());
                    }
                    pointer
                }
                ValueBinding::RunResult {
                    node_id: source,
                    pointer,
                } => {
                    if source == node_id
                        || !nodes.get(source).is_some_and(|n| {
                            matches!(
                                n.kind,
                                GraphNodeKind::Source { .. } | GraphNodeKind::Action { .. }
                            )
                        })
                    {
                        return Err(invalid());
                    }
                    let mut ancestors = BTreeSet::new();
                    let mut pending = vec![node_id.clone()];
                    while let Some(target) = pending.pop() {
                        for edge in document
                            .graph
                            .edges
                            .iter()
                            .filter(|edge| edge.target.node_id == target)
                        {
                            if ancestors.insert(edge.source.node_id.clone()) {
                                pending.push(edge.source.node_id.clone());
                            }
                        }
                    }
                    if !ancestors.contains(source) {
                        return Err(invalid());
                    }
                    pointer
                }
            };
            if pointer.len() > 512 || (!pointer.is_empty() && !pointer.starts_with('/')) {
                return Err(invalid());
            }
        }
    }
    for (node_id, instance_id) in &document.graph.node_instances {
        if !nodes.get(node_id).is_some_and(|n| {
            matches!(
                n.kind,
                GraphNodeKind::Source { .. }
                    | GraphNodeKind::Action { .. }
                    | GraphNodeKind::Feedback { .. }
            )
        }) || !document
            .plugin_instances
            .get(instance_id)
            .is_some_and(|instance| {
                instance.instance_id == *instance_id && instance.validate().is_ok()
            })
        {
            return Err(GraphValidationError::InvalidNodeShape {
                node_id: node_id.clone(),
                node_kind: "configured instance",
            });
        }
    }
    Ok(())
}
