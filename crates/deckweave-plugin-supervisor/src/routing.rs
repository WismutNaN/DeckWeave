// SPDX-License-Identifier: GPL-3.0-or-later
//! Translation of authenticated wire endpoint paths into canonical runtime identity.

use deckweave_domain::{
    automation::ResolvedEndpointRoute,
    endpoint::{EndpointTargetRef, PortPath, RuntimeEndpointRef, StableEndpointRef},
    ids::{
        CapabilityId, DescriptorRevision, DeviceIdentity, ElementId, NodeId, PortId, ProviderId,
        ServiceId,
    },
};
use deckweave_protocol::v1::{EndpointPath, Signal, endpoint_path};
use deckweave_registry::{CapabilityRegistry, SessionIdentity};
use deckweave_workspace::EndpointBinding;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EndpointRoutingError {
    #[error("signal не содержит полный source EndpointPath")]
    MissingSource,
    #[error("endpoint target не указан")]
    MissingTarget,
    #[error("endpoint path содержит некорректное поле {field}: {reason}")]
    InvalidField { field: &'static str, reason: String },
    #[error("{side} endpoint binding {binding_id} не разрешён: {reason}")]
    UnresolvedBinding {
        binding_id: String,
        side: &'static str,
        reason: String,
    },
}

pub fn resolve_endpoint_binding(
    registry: &CapabilityRegistry,
    binding: &EndpointBinding,
) -> Result<ResolvedEndpointRoute, EndpointRoutingError> {
    let resolve = |side, endpoint| {
        registry
            .resolve(endpoint)
            .map_err(|error| EndpointRoutingError::UnresolvedBinding {
                binding_id: binding.binding_id.as_str().to_owned(),
                side,
                reason: error.to_string(),
            })
    };
    Ok(ResolvedEndpointRoute {
        binding_id: binding.binding_id.clone(),
        source: resolve("source", &binding.source)?,
        target: resolve("target", &binding.target)?,
    })
}

pub fn normalize_signal_source(
    session: &SessionIdentity,
    descriptor_revision: DescriptorRevision,
    signal: &Signal,
) -> Result<RuntimeEndpointRef, EndpointRoutingError> {
    let source = signal
        .source
        .as_ref()
        .ok_or(EndpointRoutingError::MissingSource)?;
    resolve_wire_endpoint(session, descriptor_revision, source)
}

pub fn resolve_wire_endpoint(
    session: &SessionIdentity,
    descriptor_revision: DescriptorRevision,
    path: &EndpointPath,
) -> Result<RuntimeEndpointRef, EndpointRoutingError> {
    let target = match path
        .target
        .as_ref()
        .ok_or(EndpointRoutingError::MissingTarget)?
    {
        endpoint_path::Target::DeviceIdentity(value) => EndpointTargetRef::Device {
            provider_id: parse("providerId", &path.provider_id, ProviderId::try_new)?,
            device_identity: parse("deviceIdentity", value, DeviceIdentity::try_new)?,
        },
        endpoint_path::Target::ServiceId(value) => EndpointTargetRef::Service {
            provider_id: parse("providerId", &path.provider_id, ProviderId::try_new)?,
            service_id: parse("serviceId", value, ServiceId::try_new)?,
        },
    };
    Ok(RuntimeEndpointRef {
        stable: StableEndpointRef {
            package_id: session.package_id.clone(),
            node_id: parse("nodeId", &path.node_id, NodeId::try_new)?,
            target,
            port: PortPath {
                element_id: parse("elementId", &path.element_id, ElementId::try_new)?,
                capability_id: parse("capabilityId", &path.capability_id, CapabilityId::try_new)?,
                port_id: parse("portId", &path.port_id, PortId::try_new)?,
            },
        },
        runtime_group_id: session.runtime_group_id.clone(),
        plugin_session_id: session.plugin_session_id.clone(),
        session_generation: session.generation,
        descriptor_revision,
    })
}

fn parse<T, E: std::fmt::Display>(
    field: &'static str,
    value: &str,
    parser: impl FnOnce(String) -> Result<T, E>,
) -> Result<T, EndpointRoutingError> {
    parser(value.to_owned()).map_err(|error| EndpointRoutingError::InvalidField {
        field,
        reason: error.to_string(),
    })
}
