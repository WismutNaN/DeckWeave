// SPDX-License-Identifier: GPL-3.0-or-later

use super::compiler::GraphCompileError;
use crate::{GraphEdge, GraphNode};
use deckweave_domain::ids::GraphNodeId;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn topological_order(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
) -> Result<Vec<GraphNodeId>, GraphCompileError> {
    let mut indegree: BTreeMap<_, usize> = nodes.iter().map(|node| (node.id.clone(), 0)).collect();
    let mut outgoing: BTreeMap<GraphNodeId, Vec<GraphNodeId>> = BTreeMap::new();
    for edge in edges {
        let degree = indegree.get_mut(&edge.target.node_id).ok_or_else(|| {
            GraphCompileError::InvalidTopology {
                node_id: edge.target.node_id.clone(),
            }
        })?;
        *degree += 1;
        outgoing
            .entry(edge.source.node_id.clone())
            .or_default()
            .push(edge.target.node_id.clone());
    }
    for targets in outgoing.values_mut() {
        targets.sort();
    }
    let mut ready: BTreeSet<_> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect();
    let mut ordered = Vec::with_capacity(nodes.len());
    while let Some(node) = ready.pop_first() {
        ordered.push(node.clone());
        if let Some(targets) = outgoing.get(&node) {
            for target in targets {
                let degree =
                    indegree
                        .get_mut(target)
                        .ok_or_else(|| GraphCompileError::InvalidTopology {
                            node_id: target.clone(),
                        })?;
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(target.clone());
                }
            }
        }
    }
    if ordered.len() != nodes.len() {
        return Err(GraphCompileError::Cycle {
            nodes: indegree
                .into_iter()
                .filter(|(_, degree)| *degree > 0)
                .map(|(node, _)| node)
                .collect(),
        });
    }
    Ok(ordered)
}
