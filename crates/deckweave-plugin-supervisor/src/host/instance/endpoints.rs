// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl RuntimeInstance {
    pub(in crate::host::instance) fn stable_endpoints(
        &self,
    ) -> Vec<deckweave_domain::endpoint::StableEndpointRef> {
        self.nodes
            .iter()
            .flat_map(|node| {
                let package_id = self.controller.health().key.package_id;
                let node_id = node.node_id.clone();
                node.devices
                    .iter()
                    .flat_map({
                        let package_id = package_id.clone();
                        let node_id = node_id.clone();
                        move |device| {
                            let target = deckweave_domain::endpoint::EndpointTargetRef::Device {
                                provider_id: device.provider_id.clone(),
                                device_identity: device.identity.clone(),
                            };
                            device.elements.iter().flat_map({
                                let package_id = package_id.clone();
                                let node_id = node_id.clone();
                                move |element| {
                                    element.capabilities.iter().flat_map({
                                        let package_id = package_id.clone();
                                        let node_id = node_id.clone();
                                        let target = target.clone();
                                        move |capability| {
                                            capability.ports.iter().map({
                                                let package_id = package_id.clone();
                                                let node_id = node_id.clone();
                                                let target = target.clone();
                                                let element_id = element.element_id.clone();
                                                let capability_id =
                                                    capability.capability_id.clone();
                                                move |port| {
                                                    deckweave_domain::endpoint::StableEndpointRef {
                                                        package_id: package_id.clone(),
                                                        node_id: node_id.clone(),
                                                        target: target.clone(),
                                                        port:
                                                            deckweave_domain::endpoint::PortPath {
                                                                element_id: element_id.clone(),
                                                                capability_id: capability_id
                                                                    .clone(),
                                                                port_id: port.port_id.clone(),
                                                            },
                                                    }
                                                }
                                            })
                                        }
                                    })
                                }
                            })
                        }
                    })
                    .chain(node.services.iter().flat_map({
                        let package_id = package_id.clone();
                        let node_id = node_id.clone();
                        move |service| {
                            let target = deckweave_domain::endpoint::EndpointTargetRef::Service {
                                provider_id: service.provider_id.clone(),
                                service_id: service.service_id.clone(),
                            };
                            service.elements.iter().flat_map({
                                let package_id = package_id.clone();
                                let node_id = node_id.clone();
                                move |element| {
                                    element.capabilities.iter().flat_map({
                                        let package_id = package_id.clone();
                                        let node_id = node_id.clone();
                                        let target = target.clone();
                                        move |capability| {
                                            capability.ports.iter().map({
                                                let package_id = package_id.clone();
                                                let node_id = node_id.clone();
                                                let target = target.clone();
                                                let element_id = element.element_id.clone();
                                                let capability_id =
                                                    capability.capability_id.clone();
                                                move |port| {
                                                    deckweave_domain::endpoint::StableEndpointRef {
                                                        package_id: package_id.clone(),
                                                        node_id: node_id.clone(),
                                                        target: target.clone(),
                                                        port:
                                                            deckweave_domain::endpoint::PortPath {
                                                                element_id: element_id.clone(),
                                                                capability_id: capability_id
                                                                    .clone(),
                                                                port_id: port.port_id.clone(),
                                                            },
                                                    }
                                                }
                                            })
                                        }
                                    })
                                }
                            })
                        }
                    }))
            })
            .collect()
    }
}
