// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    GraphDocument, GraphEdgeId, GraphEdgeKind, GraphNode, GraphNodeKind, GraphPort, GraphPortId,
    GraphPortKind, GraphPortRole,
};
use deckweave_domain::{ids::GraphNodeId, schema::SchemaRef};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const SUPPORTED_GRAPH_MAJOR: u32 = 1;
const SUPPORTED_GRAPH_MINOR: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphLimits {
    pub maximum_document_bytes: usize,
    pub maximum_payload_bytes: usize,
    pub maximum_nodes: usize,
    pub maximum_edges: usize,
    pub maximum_ports_per_node: usize,
    pub maximum_queued_events: usize,
    pub maximum_pending_runs: usize,
    pub maximum_pending_invocations: usize,
    pub maximum_timers: usize,
    pub maximum_trace_entries: usize,
}

impl Default for GraphLimits {
    fn default() -> Self {
        Self {
            maximum_document_bytes: 1024 * 1024,
            maximum_payload_bytes: 1024 * 1024,
            maximum_nodes: 4_096,
            maximum_edges: 8_192,
            maximum_ports_per_node: 64,
            maximum_queued_events: 8_192,
            maximum_pending_runs: 1_024,
            maximum_pending_invocations: 4_096,
            maximum_timers: 4_096,
            maximum_trace_entries: 32_768,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphLimitKind {
    DocumentBytes,
    PayloadBytes,
    Nodes,
    Edges,
    PortsPerNode,
    QueuedEvents,
    PendingRuns,
    PendingInvocations,
    Timers,
    TraceEntries,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GraphValidationError {
    #[error("unsupported graph format {major}.{minor}")]
    UnsupportedVersion { major: u32, minor: u32 },
    #[error("graph limit {kind:?} exceeded: {actual} > {maximum}")]
    LimitExceeded {
        kind: GraphLimitKind,
        maximum: usize,
        actual: usize,
    },
    #[error("graph limit {kind:?} must be greater than zero")]
    InvalidLimit { kind: GraphLimitKind },
    #[error("duplicate graph node {node_id}")]
    DuplicateNode { node_id: GraphNodeId },
    #[error("duplicate graph edge {edge_id}")]
    DuplicateEdge { edge_id: GraphEdgeId },
    #[error("duplicate graph port {port_id} on node {node_id}")]
    DuplicatePort {
        node_id: GraphNodeId,
        port_id: GraphPortId,
    },
    #[error("node {node_id} has an invalid port shape for {node_kind}")]
    InvalidNodeShape {
        node_id: GraphNodeId,
        node_kind: &'static str,
    },
    #[error("edge {edge_id} references missing node {node_id}")]
    MissingNode {
        edge_id: GraphEdgeId,
        node_id: GraphNodeId,
    },
    #[error("edge {edge_id} references missing {direction} port {port_id} on node {node_id}")]
    MissingPort {
        edge_id: GraphEdgeId,
        node_id: GraphNodeId,
        port_id: GraphPortId,
        direction: &'static str,
    },
    #[error("edge {edge_id} kind does not match its ports")]
    PortKindMismatch { edge_id: GraphEdgeId },
    #[error("edge {edge_id} joins incompatible schemas")]
    SchemaMismatch {
        edge_id: GraphEdgeId,
        source_schema: SchemaRef,
        target_schema: SchemaRef,
    },
    #[error("layout references missing node {node_id}")]
    UnknownLayoutNode { node_id: GraphNodeId },
    #[error("layout for node {node_id} contains a non-finite coordinate")]
    InvalidLayoutCoordinate { node_id: GraphNodeId },
    #[error("graph document cannot be serialized for size validation: {0}")]
    Serialization(String),
}

impl GraphLimits {
    pub fn validate(&self) -> Result<(), GraphValidationError> {
        for (kind, value) in [
            (GraphLimitKind::DocumentBytes, self.maximum_document_bytes),
            (GraphLimitKind::PayloadBytes, self.maximum_payload_bytes),
            (GraphLimitKind::Nodes, self.maximum_nodes),
            (GraphLimitKind::Edges, self.maximum_edges),
            (GraphLimitKind::PortsPerNode, self.maximum_ports_per_node),
            (GraphLimitKind::QueuedEvents, self.maximum_queued_events),
            (GraphLimitKind::PendingRuns, self.maximum_pending_runs),
            (
                GraphLimitKind::PendingInvocations,
                self.maximum_pending_invocations,
            ),
            (GraphLimitKind::Timers, self.maximum_timers),
            (GraphLimitKind::TraceEntries, self.maximum_trace_entries),
        ] {
            if value == 0 {
                return Err(GraphValidationError::InvalidLimit { kind });
            }
        }
        Ok(())
    }
}

impl GraphDocument {
    pub fn validate(&self, limits: &GraphLimits) -> Result<(), GraphValidationError> {
        limits.validate()?;
        if self.schema_version.major != SUPPORTED_GRAPH_MAJOR
            || self.schema_version.minor > SUPPORTED_GRAPH_MINOR
        {
            return Err(GraphValidationError::UnsupportedVersion {
                major: self.schema_version.major,
                minor: self.schema_version.minor,
            });
        }
        check_limit(
            GraphLimitKind::DocumentBytes,
            limits.maximum_document_bytes,
            serde_json::to_vec(self)
                .map_err(|error| GraphValidationError::Serialization(error.to_string()))?
                .len(),
        )?;
        check_limit(
            GraphLimitKind::Nodes,
            limits.maximum_nodes,
            self.graph.nodes.len(),
        )?;
        check_limit(
            GraphLimitKind::Edges,
            limits.maximum_edges,
            self.graph.edges.len(),
        )?;

        let mut nodes = BTreeMap::new();
        for node in &self.graph.nodes {
            if nodes.insert(node.id.clone(), node).is_some() {
                return Err(GraphValidationError::DuplicateNode {
                    node_id: node.id.clone(),
                });
            }
            validate_node(node, limits)?;
        }

        let mut edges = BTreeSet::new();
        for edge in &self.graph.edges {
            if !edges.insert(edge.id.clone()) {
                return Err(GraphValidationError::DuplicateEdge {
                    edge_id: edge.id.clone(),
                });
            }
            let source = nodes.get(&edge.source.node_id).ok_or_else(|| {
                GraphValidationError::MissingNode {
                    edge_id: edge.id.clone(),
                    node_id: edge.source.node_id.clone(),
                }
            })?;
            let target = nodes.get(&edge.target.node_id).ok_or_else(|| {
                GraphValidationError::MissingNode {
                    edge_id: edge.id.clone(),
                    node_id: edge.target.node_id.clone(),
                }
            })?;
            let source_port =
                find_port(&source.outputs, &edge.source.port_id).ok_or_else(|| {
                    GraphValidationError::MissingPort {
                        edge_id: edge.id.clone(),
                        node_id: source.id.clone(),
                        port_id: edge.source.port_id.clone(),
                        direction: "output",
                    }
                })?;
            let target_port = find_port(&target.inputs, &edge.target.port_id).ok_or_else(|| {
                GraphValidationError::MissingPort {
                    edge_id: edge.id.clone(),
                    node_id: target.id.clone(),
                    port_id: edge.target.port_id.clone(),
                    direction: "input",
                }
            })?;
            let expected_kind = match edge.kind {
                GraphEdgeKind::Flow => GraphPortKind::Flow,
                GraphEdgeKind::Data => GraphPortKind::Value,
            };
            if source_port.kind != expected_kind || target_port.kind != expected_kind {
                return Err(GraphValidationError::PortKindMismatch {
                    edge_id: edge.id.clone(),
                });
            }
            if source_port.schema != target_port.schema {
                return Err(GraphValidationError::SchemaMismatch {
                    edge_id: edge.id.clone(),
                    source_schema: source_port.schema.clone(),
                    target_schema: target_port.schema.clone(),
                });
            }
        }

        for (node_id, placement) in &self.layout.nodes {
            if !nodes.contains_key(node_id) {
                return Err(GraphValidationError::UnknownLayoutNode {
                    node_id: node_id.clone(),
                });
            }
            if !placement.x.is_finite() || !placement.y.is_finite() {
                return Err(GraphValidationError::InvalidLayoutCoordinate {
                    node_id: node_id.clone(),
                });
            }
        }
        Ok(())
    }
}

fn validate_node(node: &GraphNode, limits: &GraphLimits) -> Result<(), GraphValidationError> {
    check_limit(
        GraphLimitKind::PortsPerNode,
        limits.maximum_ports_per_node,
        node.inputs.len().saturating_add(node.outputs.len()),
    )?;
    let valid_shape =
        match node.kind {
            GraphNodeKind::Source { .. } => {
                node.inputs.is_empty()
                    && !node.outputs.is_empty()
                    && node.outputs.iter().all(|port| {
                        matches!(port.role, GraphPortRole::Trigger | GraphPortRole::Value)
                    })
            }
            GraphNodeKind::Feedback { .. } => {
                !node.inputs.is_empty()
                    && node.outputs.is_empty()
                    && node
                        .inputs
                        .iter()
                        .all(|port| port.role == GraphPortRole::Input)
            }
            GraphNodeKind::Action { .. } => {
                !node.inputs.is_empty()
                    && !node.outputs.is_empty()
                    && node.inputs.iter().all(|port| {
                        matches!(port.role, GraphPortRole::Trigger | GraphPortRole::Input)
                    })
                    && node.outputs.iter().all(|port| {
                        matches!(port.role, GraphPortRole::Result | GraphPortRole::Error)
                    })
            }
            GraphNodeKind::State { .. } => {
                !node.inputs.is_empty()
                    && !node.outputs.is_empty()
                    && node
                        .inputs
                        .iter()
                        .all(|port| port.role == GraphPortRole::Input)
                    && node
                        .outputs
                        .iter()
                        .all(|port| port.role == GraphPortRole::Value)
            }
            GraphNodeKind::Condition { .. } => {
                !node.inputs.is_empty()
                    && !node.outputs.is_empty()
                    && node
                        .inputs
                        .iter()
                        .all(|port| port.role == GraphPortRole::Input)
                    && node
                        .outputs
                        .iter()
                        .all(|port| matches!(port.role, GraphPortRole::True | GraphPortRole::False))
            }
        };
    if !valid_shape {
        return Err(GraphValidationError::InvalidNodeShape {
            node_id: node.id.clone(),
            node_kind: node_kind_name(&node.kind),
        });
    }
    let mut ports = BTreeSet::new();
    for port in node.inputs.iter().chain(&node.outputs) {
        let valid_port_kind = match port.role {
            GraphPortRole::Trigger | GraphPortRole::True | GraphPortRole::False => {
                port.kind == GraphPortKind::Flow
            }
            GraphPortRole::Result | GraphPortRole::Error | GraphPortRole::Value => {
                port.kind == GraphPortKind::Value
            }
            GraphPortRole::Input => true,
        };
        if !valid_port_kind {
            return Err(GraphValidationError::InvalidNodeShape {
                node_id: node.id.clone(),
                node_kind: node_kind_name(&node.kind),
            });
        }
        if !ports.insert(port.id.clone()) {
            return Err(GraphValidationError::DuplicatePort {
                node_id: node.id.clone(),
                port_id: port.id.clone(),
            });
        }
    }
    Ok(())
}

fn node_kind_name(kind: &GraphNodeKind) -> &'static str {
    match kind {
        GraphNodeKind::Source { .. } => "source",
        GraphNodeKind::Action { .. } => "action",
        GraphNodeKind::State { .. } => "state",
        GraphNodeKind::Condition { .. } => "condition",
        GraphNodeKind::Feedback { .. } => "feedback",
    }
}

fn find_port<'a>(ports: &'a [GraphPort], id: &GraphPortId) -> Option<&'a GraphPort> {
    ports.iter().find(|port| port.id == *id)
}

fn check_limit(
    kind: GraphLimitKind,
    maximum: usize,
    actual: usize,
) -> Result<(), GraphValidationError> {
    if actual > maximum {
        return Err(GraphValidationError::LimitExceeded {
            kind,
            maximum,
            actual,
        });
    }
    Ok(())
}
