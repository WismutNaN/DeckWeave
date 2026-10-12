// SPDX-License-Identifier: GPL-3.0-or-later
//! Stable endpoint identity shared by profiles, routing and runtime resolution.

use crate::ids::{
    CapabilityId, DescriptorRevision, DeviceIdentity, ElementId, NodeId, PackageId,
    PluginSessionId, PortId, ProviderId, RuntimeGroupId, ServiceId, SessionGeneration,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum EndpointTargetRef {
    Device {
        provider_id: ProviderId,
        device_identity: DeviceIdentity,
    },
    Service {
        provider_id: ProviderId,
        service_id: ServiceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortPath {
    pub element_id: ElementId,
    pub capability_id: CapabilityId,
    pub port_id: PortId,
}

/// Persisted identity. Runtime placement and session generations are excluded
/// so a binding survives process regrouping and provider restarts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StableEndpointRef {
    pub package_id: PackageId,
    pub node_id: NodeId,
    pub target: EndpointTargetRef,
    pub port: PortPath,
}

/// Runtime-resolved identity. It is never persisted in a workspace because
/// every transient field becomes stale after a provider restart or regrouping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEndpointRef {
    pub stable: StableEndpointRef,
    pub runtime_group_id: RuntimeGroupId,
    pub plugin_session_id: PluginSessionId,
    pub session_generation: SessionGeneration,
    pub descriptor_revision: DescriptorRevision,
}
