// SPDX-License-Identifier: GPL-3.0-or-later
use super::{DescriptorRevision, NodeDescriptor, NodeId, RegistryError};
use deckweave_domain::{
    capability::{ElementDescriptor, PortDescriptor},
    endpoint::{EndpointTargetRef, PortPath},
};
use std::collections::BTreeMap;

pub(super) fn validate_nodes(
    nodes: Vec<NodeDescriptor>,
    revision: DescriptorRevision,
) -> Result<BTreeMap<NodeId, NodeDescriptor>, RegistryError> {
    let mut validated = BTreeMap::new();
    for node in nodes {
        if node.descriptor_revision != revision {
            return Err(RegistryError::StaleDescriptorRevision);
        }
        node.validate()
            .map_err(|error| RegistryError::InvalidDescriptor(error.to_string()))?;
        if validated.insert(node.node_id.clone(), node).is_some() {
            return Err(RegistryError::DuplicateNode("within snapshot".into()));
        }
    }
    validate_node_set(&validated)?;
    Ok(validated)
}

pub(super) fn validate_node_set(
    nodes: &BTreeMap<NodeId, NodeDescriptor>,
) -> Result<(), RegistryError> {
    for node in nodes.values() {
        node.validate()
            .map_err(|error| RegistryError::InvalidDescriptor(error.to_string()))?;
    }
    Ok(())
}

fn find_in_elements<'a>(
    elements: &'a [ElementDescriptor],
    path: &PortPath,
) -> Option<&'a PortDescriptor> {
    elements
        .iter()
        .find(|element| element.element_id == path.element_id)?
        .capabilities
        .iter()
        .find(|capability| capability.capability_id == path.capability_id)?
        .ports
        .iter()
        .find(|port| port.port_id == path.port_id)
}

pub(super) fn find_port<'a>(
    node: &'a NodeDescriptor,
    target: &EndpointTargetRef,
    path: &PortPath,
) -> Option<&'a PortDescriptor> {
    let elements = match target {
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
    find_in_elements(elements, path)
}
