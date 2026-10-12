// SPDX-License-Identifier: GPL-3.0-or-later
//! Translation of one authenticated runtime's descriptors into binder input.

use super::RuntimeInstance;
use crate::host::HostRuntimeError;
use deckweave_automation::{CompiledEndpointContract, RuntimeBinding};
use deckweave_domain::{
    capability::{NodeDescriptor, PortDescriptor},
    endpoint::{EndpointTargetRef, RuntimeEndpointRef, StableEndpointRef},
};

pub(super) fn runtime_bindings(
    instance: &RuntimeInstance,
    contracts: &[CompiledEndpointContract],
) -> Result<Vec<RuntimeBinding>, HostRuntimeError> {
    let session = instance
        .controller
        .session()
        .ok_or_else(|| HostRuntimeError::Protocol("runtime session is missing".into()))?;
    if contracts.iter().any(|c| {
        c.endpoint.package_id == session.identity.package_id
            && c.runtime_group_id == session.identity.runtime_group_id
            && c.configured_instance_id.is_some()
            && c.port.direction == deckweave_domain::capability::PortDirection::Input
    }) && session
        .negotiated_features
        .get(deckweave_protocol::FEATURE_CONFIGURED_DISPATCH)
        .copied()
        != Some(1)
    {
        return Err(HostRuntimeError::Protocol(
            "configured dispatch is not negotiated".into(),
        ));
    }
    Ok(contracts
        .iter()
        .filter(|contract| {
            contract.endpoint.package_id == session.identity.package_id
                && contract.runtime_group_id == session.identity.runtime_group_id
                && instance.contributions.contains(&contract.contribution_id)
        })
        .filter_map(|contract| {
            let (_, _, owned) =
                instance
                    .activation_nodes
                    .iter()
                    .find(|(contribution, instance_id, _)| {
                        *contribution == contract.contribution_id
                            && *instance_id == contract.configured_instance_id
                    })?;
            find_port(owned, &contract.endpoint)?;
            find_port(&instance.nodes, &contract.endpoint).map(|port| RuntimeBinding {
                endpoint: RuntimeEndpointRef {
                    stable: contract.endpoint.clone(),
                    runtime_group_id: session.identity.runtime_group_id.clone(),
                    plugin_session_id: session.identity.plugin_session_id.clone(),
                    session_generation: session.identity.generation,
                    descriptor_revision: instance.descriptor_revision,
                },
                contribution_id: contract.contribution_id.clone(),
                port: port.clone(),
                operation: contract.operation.clone(),
            })
        })
        .collect())
}

pub(super) fn find_port<'a>(
    nodes: &'a [NodeDescriptor],
    endpoint: &StableEndpointRef,
) -> Option<&'a PortDescriptor> {
    let node = nodes.iter().find(|node| node.node_id == endpoint.node_id)?;
    let elements = match &endpoint.target {
        EndpointTargetRef::Device {
            provider_id,
            device_identity,
        } => {
            &node
                .devices
                .iter()
                .find(|device| {
                    device.provider_id == *provider_id && device.identity == *device_identity
                })?
                .elements
        }
        EndpointTargetRef::Service {
            provider_id,
            service_id,
        } => {
            &node
                .services
                .iter()
                .find(|service| {
                    service.provider_id == *provider_id && service.service_id == *service_id
                })?
                .elements
        }
    };
    elements
        .iter()
        .find(|element| element.element_id == endpoint.port.element_id)?
        .capabilities
        .iter()
        .find(|capability| capability.capability_id == endpoint.port.capability_id)?
        .ports
        .iter()
        .find(|port| port.port_id == endpoint.port.port_id)
}
