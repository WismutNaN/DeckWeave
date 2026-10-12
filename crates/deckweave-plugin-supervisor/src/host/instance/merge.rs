// SPDX-License-Identifier: GPL-3.0-or-later
//! Configured consumers may share an identical input-only service contract.
use crate::host::HostRuntimeError;
use deckweave_domain::capability::{NodeDescriptor, PortDirection};

pub(super) fn merge(
    existing: &mut NodeDescriptor,
    incoming: NodeDescriptor,
) -> Result<(), HostRuntimeError> {
    if existing.kind != incoming.kind {
        return Err(HostRuntimeError::Protocol("node kind conflict".into()));
    }
    // Devices and event/state sources must retain an unambiguous physical identity.
    existing.devices.extend(incoming.devices);
    for service in incoming.services {
        if let Some(previous) = existing
            .services
            .iter()
            .find(|s| s.provider_id == service.provider_id && s.service_id == service.service_id)
        {
            let input_only = service
                .elements
                .iter()
                .flat_map(|e| &e.capabilities)
                .flat_map(|c| &c.ports)
                .all(|p| p.direction == PortDirection::Input);
            if previous != &service || !input_only {
                return Err(HostRuntimeError::Protocol(
                    "ambiguous shared service descriptor".into(),
                ));
            }
        } else {
            existing.services.push(service);
        }
    }
    existing
        .validate()
        .map_err(|e| HostRuntimeError::Protocol(e.to_string()))
}
