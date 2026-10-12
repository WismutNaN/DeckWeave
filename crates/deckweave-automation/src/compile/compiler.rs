// SPDX-License-Identifier: GPL-3.0-or-later
mod access;
mod diagnostic;
use super::{
    catalog::{CatalogEndpoint, DescriptorCatalog},
    semantics::{
        collect_availability, collect_runtime_requirement, compile_schema_contracts, contract,
        runtime_requirement_order, validate_action_ports, validate_endpoint_ports,
        validate_node_values,
    },
    topology::topological_order,
};
use crate::{
    CompileFence, CompiledExecutionPlan, EndpointAvailabilityPolicy, ExecutableGraph,
    GraphDocument, GraphLimits, GraphNode, GraphNodeKind, GraphValidationError, PlanCompileError,
    PlanCompiler, RecoveryCompileInput, RuntimeRequirement, SourceSubscription, StateSlot,
};
use deckweave_domain::{
    capability::{PortAvailability, PortDirection},
    endpoint::StableEndpointRef,
    ids::{GraphNodeId, NamespacedKind},
};
use deckweave_workspace::{PackageLockEntry, WorkspaceCandidate};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

const COMPILER_ID: &str = "org.deckweave.compiler.graph-v1";
const COMPILER_VERSION: &str = "1";

#[derive(Debug, Clone)]
pub struct GraphPlanCompiler {
    catalog: DescriptorCatalog,
    limits: GraphLimits,
    access: CompileAccess,
}

#[derive(Debug, Clone, Default)]
pub struct CompileAccess {
    pub endpoint_permissions:
        std::collections::BTreeMap<StableEndpointRef, BTreeSet<NamespacedKind>>,
    pub granted_permissions: BTreeSet<NamespacedKind>,
    pub available_resources: BTreeSet<NamespacedKind>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum GraphCompileError {
    #[error("node {node_id} has invalid argument {field}")]
    InvalidArgument { node_id: GraphNodeId, field: String },
    #[error("graph document cannot be decoded: {0}")]
    InvalidDocument(String),
    #[error(transparent)]
    InvalidGraph(#[from] GraphValidationError),
    #[error("graph contains a cycle involving {nodes:?}")]
    Cycle { nodes: Vec<GraphNodeId> },
    #[error("validated graph topology lost node {node_id}")]
    InvalidTopology { node_id: GraphNodeId },
    #[error("node {node_id} references endpoint absent from the descriptor catalog")]
    UnresolvedEndpoint { node_id: GraphNodeId },
    #[error("node {node_id} references an unsupported endpoint")]
    UnsupportedEndpoint { node_id: GraphNodeId },
    #[error("node {node_id} requires an endpoint that is not currently available")]
    RequiredEndpointUnavailable { node_id: GraphNodeId },
    #[error("node {node_id} expects endpoint direction {expected:?}")]
    DirectionMismatch {
        node_id: GraphNodeId,
        expected: PortDirection,
    },
    #[error("node {node_id} references unknown schema")]
    UnresolvedSchema { node_id: GraphNodeId },
    #[error("node {node_id} schema/unit/range contract does not match catalog")]
    SchemaConstraintMismatch { node_id: GraphNodeId },
    #[error("node {node_id} has no matching operation descriptor")]
    OperationMismatch { node_id: GraphNodeId },
    #[error("node {node_id} is missing compile-time permission {scope}")]
    MissingPermission {
        node_id: GraphNodeId,
        scope: NamespacedKind,
    },
    #[error("node {node_id} is missing compile-time resource {scope}")]
    MissingResource {
        node_id: GraphNodeId,
        scope: NamespacedKind,
    },
    #[error("node {node_id} contains a value incompatible with its schema")]
    InvalidValue { node_id: GraphNodeId },
    #[error("package lock does not contain the exact catalog artifact for {package_id}")]
    PackageLockMismatch { package_id: String },
    #[error("state slot count exceeds u32")]
    StateSlotOverflow,
    #[error("compiled graph serialization failed: {0}")]
    Serialization(String),
}

impl GraphPlanCompiler {
    pub fn catalog(&self) -> &DescriptorCatalog {
        &self.catalog
    }
    pub fn new(catalog: DescriptorCatalog, limits: GraphLimits) -> Result<Self, GraphCompileError> {
        Self::with_access(catalog, limits, CompileAccess::default())
    }

    pub fn with_access(
        catalog: DescriptorCatalog,
        limits: GraphLimits,
        access: CompileAccess,
    ) -> Result<Self, GraphCompileError> {
        limits.validate()?;
        Ok(Self {
            catalog,
            limits,
            access,
        })
    }

    pub fn compile_document(
        &self,
        document: &Value,
        package_lock: &[PackageLockEntry],
        package_lock_sha256: &str,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, GraphCompileError> {
        let graph: GraphDocument = serde_json::from_value(document.clone())
            .map_err(|error| GraphCompileError::InvalidDocument(error.to_string()))?;
        graph.validate(&self.limits)?;
        self.compile_graph(graph, package_lock, package_lock_sha256, fence)
    }

    fn compile_graph(
        &self,
        graph: GraphDocument,
        package_lock: &[PackageLockEntry],
        package_lock_sha256: &str,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, GraphCompileError> {
        let mut nodes = graph.graph.nodes;
        let mut edges = graph.graph.edges;
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        let topological_order = topological_order(&nodes, &edges)?;

        let mut sources = Vec::new();
        let mut states = Vec::new();
        let mut contracts = Vec::new();
        let mut diagnostics = Vec::new();
        let mut requirements = Vec::new();
        let mut requirement_keys = BTreeSet::new();

        for node in &nodes {
            validate_node_values(node, &self.catalog)?;
            let configure = |entry| {
                super::instances::configure_endpoint(
                    node,
                    entry,
                    &self.catalog,
                    &graph.plugin_instances,
                    graph.graph.node_instances.get(&node.id),
                )
            };
            match &node.kind {
                GraphNodeKind::Source {
                    endpoint,
                    availability,
                } => {
                    let entry = self.require_endpoint(
                        node,
                        endpoint,
                        PortDirection::Output,
                        *availability,
                    )?;
                    let entry = &configure(entry)?;
                    validate_endpoint_ports(node, entry, &self.catalog)?;
                    collect_runtime_requirement(
                        entry,
                        package_lock,
                        &mut requirement_keys,
                        &mut requirements,
                    )?;
                    collect_availability(node, entry, *availability, &mut diagnostics);
                    sources.push(SourceSubscription {
                        node_id: node.id.clone(),
                        endpoint: endpoint.clone(),
                    });
                    contracts.push(contract(node, entry));
                }
                GraphNodeKind::Action {
                    endpoint,
                    operation_id,
                    availability,
                } => {
                    let entry =
                        self.require_endpoint(node, endpoint, PortDirection::Input, *availability)?;
                    let entry = &configure(entry)?;
                    let operation = entry.operation.as_ref().ok_or_else(|| {
                        GraphCompileError::OperationMismatch {
                            node_id: node.id.clone(),
                        }
                    })?;
                    if operation.operation_id != *operation_id {
                        return Err(GraphCompileError::OperationMismatch {
                            node_id: node.id.clone(),
                        });
                    }
                    validate_action_ports(
                        node,
                        operation,
                        &self.catalog,
                        graph.graph.action_arguments.get(&node.id),
                    )?;
                    if let Some(arguments) = graph.graph.action_arguments.get(&node.id) {
                        super::arguments::validate(node, arguments, &nodes, entry, &self.catalog)?;
                    }
                    collect_runtime_requirement(
                        entry,
                        package_lock,
                        &mut requirement_keys,
                        &mut requirements,
                    )?;
                    collect_availability(node, entry, *availability, &mut diagnostics);
                    contracts.push(contract(node, entry));
                }
                GraphNodeKind::Feedback {
                    endpoint,
                    availability,
                } => {
                    let entry =
                        self.require_endpoint(node, endpoint, PortDirection::Input, *availability)?;
                    let entry = &configure(entry)?;
                    validate_endpoint_ports(node, entry, &self.catalog)?;
                    collect_runtime_requirement(
                        entry,
                        package_lock,
                        &mut requirement_keys,
                        &mut requirements,
                    )?;
                    collect_availability(node, entry, *availability, &mut diagnostics);
                    contracts.push(contract(node, entry));
                }
                GraphNodeKind::State {
                    state_key,
                    initial_value,
                } => {
                    let output = node.outputs.first().ok_or_else(|| {
                        GraphCompileError::SchemaConstraintMismatch {
                            node_id: node.id.clone(),
                        }
                    })?;
                    states.push(StateSlot {
                        index: u32::try_from(states.len())
                            .map_err(|_| GraphCompileError::StateSlotOverflow)?,
                        node_id: node.id.clone(),
                        key: state_key.clone(),
                        schema: output.schema.clone(),
                        initial_value: initial_value.clone(),
                    });
                }
                GraphNodeKind::Condition { .. } => {}
            }
        }

        sources.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        contracts.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        requirements.sort_by(runtime_requirement_order);
        let schema_contracts = compile_schema_contracts(&nodes, &self.catalog)?;
        let executable = ExecutableGraph {
            action_arguments: graph.graph.action_arguments,
            format: graph.schema_version,
            nodes,
            edges,
            topological_order,
            source_subscriptions: sources,
            state_slots: states,
            endpoint_contracts: contracts,
            schema_contracts,
        };
        let semantic_sha256 =
            self.semantic_hash(&executable, &requirements, package_lock_sha256, fence)?;
        Ok(CompiledExecutionPlan::new_graph(
            COMPILER_ID.into(),
            COMPILER_VERSION.into(),
            semantic_sha256,
            requirements,
            executable,
            diagnostics,
        ))
    }

    fn require_endpoint<'a>(
        &'a self,
        node: &GraphNode,
        endpoint: &StableEndpointRef,
        direction: PortDirection,
        availability: EndpointAvailabilityPolicy,
    ) -> Result<&'a CatalogEndpoint, GraphCompileError> {
        let entry = self.catalog.endpoint(endpoint).ok_or_else(|| {
            GraphCompileError::UnresolvedEndpoint {
                node_id: node.id.clone(),
            }
        })?;
        if entry.port.direction != direction {
            return Err(GraphCompileError::DirectionMismatch {
                node_id: node.id.clone(),
                expected: direction,
            });
        }
        if entry.port.availability.status == PortAvailability::Unsupported {
            return Err(GraphCompileError::UnsupportedEndpoint {
                node_id: node.id.clone(),
            });
        }
        if availability == EndpointAvailabilityPolicy::Required
            && entry.port.availability.status != PortAvailability::Available
        {
            return Err(GraphCompileError::RequiredEndpointUnavailable {
                node_id: node.id.clone(),
            });
        }
        self.validate_access(node, entry)?;
        Ok(entry)
    }

    fn semantic_hash(
        &self,
        executable: &ExecutableGraph,
        requirements: &[RuntimeRequirement],
        package_lock_sha256: &str,
        fence: &CompileFence,
    ) -> Result<String, GraphCompileError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SemanticInput<'a> {
            compiler_id: &'static str,
            compiler_version: &'static str,
            executable: &'a ExecutableGraph,
            requirements: &'a [RuntimeRequirement],
            catalog_sha256: &'a str,
            package_lock_sha256: &'a str,
            fence: &'a CompileFence,
        }
        serde_json::to_vec(&SemanticInput {
            compiler_id: COMPILER_ID,
            compiler_version: COMPILER_VERSION,
            executable,
            requirements,
            catalog_sha256: self.catalog.semantic_sha256(),
            package_lock_sha256,
            fence,
        })
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| GraphCompileError::Serialization(error.to_string()))
    }
}

impl PlanCompiler for GraphPlanCompiler {
    fn compile(
        &self,
        candidate: &WorkspaceCandidate,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError> {
        self.compile_document(
            candidate.document(),
            candidate.package_lock(),
            candidate.package_lock_sha256(),
            fence,
        )
        .map_err(GraphCompileError::into_plan_error)
    }

    fn compile_recovery(
        &self,
        input: RecoveryCompileInput<'_>,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError> {
        self.compile_document(
            &input.revision.document,
            &input.revision.package_lock,
            input.package_lock_sha256,
            fence,
        )
        .map_err(GraphCompileError::into_plan_error)
    }
}
