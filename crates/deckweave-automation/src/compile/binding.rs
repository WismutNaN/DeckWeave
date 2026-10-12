// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    BoundEndpoint, BoundExecutionGraph, CompiledExecutionPlan, ExecutableGraph, RuntimeBinding,
};
use deckweave_domain::{
    capability::PortAvailability, endpoint::StableEndpointRef, ids::GraphNodeId,
};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Default)]
pub struct RuntimeBinder;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeBindError {
    #[error("compiled plan has no executable graph")]
    MissingExecutableGraph,
    #[error("runtime binding snapshot contains duplicate endpoint")]
    DuplicateEndpoint,
    #[error("node {node_id} needs discovery before runtime binding")]
    NeedsDiscovery { node_id: GraphNodeId },
    #[error("node {node_id} resolved to a stale or incompatible descriptor")]
    IncompatibleDescriptor { node_id: GraphNodeId },
    #[error("node {node_id} endpoint is not currently available")]
    Unavailable { node_id: GraphNodeId },
}

impl RuntimeBinder {
    pub fn bind_plan(
        &self,
        plan: &CompiledExecutionPlan,
        bindings: &[RuntimeBinding],
    ) -> Result<BoundExecutionGraph, RuntimeBindError> {
        let graph = plan
            .executable_graph()
            .ok_or(RuntimeBindError::MissingExecutableGraph)?;
        self.bind(graph, bindings)
    }

    pub fn bind(
        &self,
        graph: &ExecutableGraph,
        bindings: &[RuntimeBinding],
    ) -> Result<BoundExecutionGraph, RuntimeBindError> {
        let mut by_endpoint: BTreeMap<StableEndpointRef, &RuntimeBinding> = BTreeMap::new();
        for binding in bindings {
            if by_endpoint
                .insert(binding.endpoint.stable.clone(), binding)
                .is_some()
            {
                return Err(RuntimeBindError::DuplicateEndpoint);
            }
        }

        let mut endpoints = Vec::with_capacity(graph.endpoint_contracts.len());
        for contract in &graph.endpoint_contracts {
            let binding = by_endpoint.get(&contract.endpoint).ok_or_else(|| {
                RuntimeBindError::NeedsDiscovery {
                    node_id: contract.node_id.clone(),
                }
            })?;
            if binding.endpoint.stable != contract.endpoint
                || binding.endpoint.runtime_group_id != contract.runtime_group_id
                || binding.contribution_id != contract.contribution_id
                || binding.endpoint.descriptor_revision != binding.port.descriptor_revision
                || binding.port.port_id != contract.port.port_id
                || binding.port.kind != contract.port.kind
                || binding.port.direction != contract.port.direction
                || binding.port.semantics != contract.port.semantics
                || binding.port.schema != contract.port.schema
                || binding.port.compatible_versions != contract.port.compatible_versions
                || binding.port.unit != contract.port.unit
                || binding.port.range != contract.port.range
                || binding.port.required_permissions != contract.port.required_permissions
                || binding.port.coordinate_space != contract.port.coordinate_space
                || binding.port.lifetime != contract.port.lifetime
                || binding.operation != contract.operation
            {
                return Err(RuntimeBindError::IncompatibleDescriptor {
                    node_id: contract.node_id.clone(),
                });
            }
            if !matches!(
                binding.port.availability.status,
                PortAvailability::Available | PortAvailability::Degraded
            ) {
                return Err(RuntimeBindError::Unavailable {
                    node_id: contract.node_id.clone(),
                });
            }
            endpoints.push(BoundEndpoint {
                node_id: contract.node_id.clone(),
                endpoint: binding.endpoint.clone(),
            });
        }
        endpoints.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        Ok(BoundExecutionGraph { endpoints })
    }
}
