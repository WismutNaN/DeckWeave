// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::virtual_control::*;
use deckweave_domain::{
    capability::DeviceKind,
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
};
use std::collections::BTreeSet;

impl VirtualControlPort for ProductionRuntimeCoordinator {
    fn snapshot(&self) -> Result<VirtualSurfaceSnapshot, VirtualControlError> {
        let snapshot = self
            .registry_snapshot()
            .map_err(|_| VirtualControlError::Unavailable)?;
        let mut allowed = BTreeSet::new();
        for node in snapshot.nodes {
            for device in node
                .descriptor
                .devices
                .iter()
                .filter(|d| d.kind == DeviceKind::Virtual)
            {
                for element in &device.elements {
                    for capability in &element.capabilities {
                        for port in &capability.ports {
                            if port
                                .tags
                                .iter()
                                .any(|t| t == "org.deckweave.virtual.display")
                                && port.kind.as_str() == "ValueOut"
                            {
                                allowed.insert(StableEndpointRef {
                                    package_id: node.session.package_id.clone(),
                                    node_id: node.descriptor.node_id.clone(),
                                    target: EndpointTargetRef::Device {
                                        provider_id: device.provider_id.clone(),
                                        device_identity: device.identity.clone(),
                                    },
                                    port: PortPath {
                                        element_id: element.element_id.clone(),
                                        capability_id: capability.capability_id.clone(),
                                        port_id: port.port_id.clone(),
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
        self.execution
            .virtual_surface(&allowed)
            .map_err(|_| VirtualControlError::Unavailable)
    }
    fn click(&self, request: &VirtualClickRequest) -> Result<(), VirtualControlError> {
        let state = self
            .state
            .lock()
            .map_err(|_| VirtualControlError::Unavailable)?;
        if state.dispatcher_closed {
            return Err(VirtualControlError::Unavailable);
        }
        let commit = self
            .operations
            .active_commit()
            .map_err(|_| VirtualControlError::Unavailable)?;
        if commit.is_none_or(|c| c.candidate != request.expected_active) {
            return Err(VirtualControlError::Stale);
        }
        let endpoint = state
            .active_binding
            .as_ref()
            .and_then(|b| {
                b.endpoints
                    .iter()
                    .find(|b| b.endpoint.stable == request.endpoint)
            })
            .map(|b| &b.endpoint)
            .ok_or(VirtualControlError::Unsupported)?;
        if endpoint.session_generation != request.session_generation {
            return Err(VirtualControlError::Stale);
        }
        let EndpointTargetRef::Device {
            provider_id,
            device_identity,
        } = &endpoint.stable.target
        else {
            return Err(VirtualControlError::Unsupported);
        };
        let registry = self
            .registry_snapshot()
            .map_err(|_| VirtualControlError::Unavailable)?;
        let virtual_device = registry
            .nodes
            .iter()
            .filter(|n| {
                n.session.package_id == request.endpoint.package_id
                    && n.session.generation == request.session_generation
                    && n.descriptor.node_id == request.endpoint.node_id
            })
            .flat_map(|n| &n.descriptor.devices)
            .any(|d| {
                d.kind == DeviceKind::Virtual
                    && d.provider_id == *provider_id
                    && d.identity == *device_identity
            });
        if !virtual_device {
            return Err(VirtualControlError::Unsupported);
        }
        self.execution
            .virtual_click(request.activation_epoch, endpoint)
            .map_err(|_| VirtualControlError::Stale)
    }
}
