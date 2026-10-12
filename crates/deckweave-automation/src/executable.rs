// SPDX-License-Identifier: GPL-3.0-or-later
//! Immutable compiler output consumed by the scheduler and runtime binder.

use crate::{GraphEdge, GraphFormatVersion, GraphNode, SchemaValueKind};
use deckweave_domain::{
    automation::OperationDescriptor,
    capability::PortDescriptor,
    endpoint::{RuntimeEndpointRef, StableEndpointRef},
    ids::{ContributionId, GraphNodeId, NamespacedKind, RuntimeGroupId},
    schema::SchemaRef,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableGraph {
    pub format: GraphFormatVersion,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub topological_order: Vec<GraphNodeId>,
    pub source_subscriptions: Vec<SourceSubscription>,
    pub state_slots: Vec<StateSlot>,
    pub endpoint_contracts: Vec<CompiledEndpointContract>,
    #[serde(default)]
    pub schema_contracts: Vec<CompiledSchemaContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSubscription {
    pub node_id: GraphNodeId,
    pub endpoint: StableEndpointRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSlot {
    pub index: u32,
    pub node_id: GraphNodeId,
    pub key: NamespacedKind,
    pub schema: SchemaRef,
    pub initial_value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledEndpointContract {
    pub node_id: GraphNodeId,
    pub endpoint: StableEndpointRef,
    pub runtime_group_id: RuntimeGroupId,
    pub contribution_id: ContributionId,
    pub port: PortDescriptor,
    pub operation: Option<OperationDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledSchemaContract {
    pub schema: SchemaRef,
    pub value_kind: SchemaValueKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileDiagnosticCode {
    NeedsRuntimeAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileDiagnostic {
    pub code: CompileDiagnosticCode,
    pub node_id: GraphNodeId,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeBinding {
    pub endpoint: RuntimeEndpointRef,
    pub contribution_id: ContributionId,
    pub port: PortDescriptor,
    pub operation: Option<OperationDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEndpoint {
    pub node_id: GraphNodeId,
    pub endpoint: RuntimeEndpointRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundExecutionGraph {
    pub endpoints: Vec<BoundEndpoint>,
}
