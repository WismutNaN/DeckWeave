// SPDX-License-Identifier: GPL-3.0-or-later
use super::{DescriptorRevision, NodeDescriptor, NodeId, PortId, RegistryError};
use deckweave_domain::capability::PortDescriptor;
use std::collections::{BTreeMap, BTreeSet};

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
    let mut ports = BTreeSet::new();
    for node in nodes.values() {
        for port in node_ports(node) {
            let key = (node.node_id.as_str(), port.port_id.as_str());
            if !ports.insert(key) {
                return Err(RegistryError::InvalidDescriptor(format!(
                    "duplicate port {} in node {}",
                    port.port_id, node.node_id
                )));
            }
        }
    }
    Ok(())
}

fn node_ports(node: &NodeDescriptor) -> impl Iterator<Item = &PortDescriptor> {
    node.devices
        .iter()
        .flat_map(|device| &device.elements)
        .chain(node.services.iter().flat_map(|service| &service.elements))
        .flat_map(|element| &element.capabilities)
        .flat_map(|capability| &capability.ports)
}

pub(super) fn find_port<'a>(
    node: &'a NodeDescriptor,
    port_id: &PortId,
) -> Option<&'a PortDescriptor> {
    node_ports(node).find(|port| port.port_id == *port_id)
}
