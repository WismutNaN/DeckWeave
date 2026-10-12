// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    ids::{
        CapabilityId, DescriptorRevision, DeviceIdentity, ElementId, NamespacedKind, NodeId,
        PortId, ProviderId, ServiceId, SessionGeneration,
    },
    schema::{ExtensionMap, SchemaRef, SchemaVersionRange},
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

mod signal;
pub use signal::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PortKind(String);

impl PortKind {
    pub const EVENT_OUT: &'static str = "EventOut";
    pub const VALUE_OUT: &'static str = "ValueOut";
    pub const STATE_OUT: &'static str = "StateOut";
    pub const COMMAND_IN: &'static str = "CommandIn";
    pub const VALUE_IN: &'static str = "ValueIn";
    pub const FRAME_IN: &'static str = "FrameIn";
    pub const HAPTIC_IN: &'static str = "HapticIn";
    pub const STREAM_OUT: &'static str = "StreamOut";
    pub const STREAM_IN: &'static str = "StreamIn";

    pub fn try_new(value: impl Into<String>) -> Result<Self, DescriptorError> {
        let value = value.into();
        let standard = [
            Self::EVENT_OUT,
            Self::VALUE_OUT,
            Self::STATE_OUT,
            Self::COMMAND_IN,
            Self::VALUE_IN,
            Self::FRAME_IN,
            Self::HAPTIC_IN,
            Self::STREAM_OUT,
            Self::STREAM_IN,
        ];
        if !standard.contains(&value.as_str()) && NamespacedKind::try_new(value.clone()).is_err() {
            return Err(DescriptorError::InvalidPortKind(value));
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PortKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortSemantics {
    Event,
    Value,
    State,
    Command,
    Frame,
    Haptic,
    Stream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortAvailability {
    Available,
    Unsupported,
    Unavailable,
    PermissionMissing,
    ResourceBusy,
    Disconnected,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortLifetime {
    Always,
    Dynamic,
    Requestable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericRange {
    pub minimum: f64,
    pub maximum: f64,
    pub center: Option<f64>,
    pub precision: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateHints {
    pub preferred_hz: Option<f64>,
    pub maximum_hz: Option<f64>,
    pub maximum_latency_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityDescriptor {
    pub status: PortAvailability,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortDescriptor {
    pub port_id: PortId,
    pub kind: PortKind,
    pub direction: PortDirection,
    pub semantics: PortSemantics,
    pub schema: SchemaRef,
    pub compatible_versions: SchemaVersionRange,
    pub unit: Option<String>,
    pub range: Option<NumericRange>,
    pub coordinate_space: Option<NamespacedKind>,
    pub rate: Option<RateHints>,
    #[serde(default)]
    pub required_permissions: Vec<NamespacedKind>,
    pub availability: AvailabilityDescriptor,
    pub lifetime: PortLifetime,
    pub session_generation: Option<SessionGeneration>,
    pub descriptor_revision: DescriptorRevision,
    pub template_id: Option<PortId>,
    #[serde(default)]
    pub tags: BTreeSet<String>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDescriptor {
    pub capability_id: CapabilityId,
    pub kind: NamespacedKind,
    #[serde(default)]
    pub ports: Vec<PortDescriptor>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementDescriptor {
    pub element_id: ElementId,
    pub label: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<CapabilityDescriptor>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Physical,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityEvidence {
    pub kind: NamespacedKind,
    pub value: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDescriptor {
    pub identity: DeviceIdentity,
    pub provider_id: ProviderId,
    pub kind: DeviceKind,
    pub label: String,
    #[serde(default)]
    pub evidence: Vec<IdentityEvidence>,
    #[serde(default)]
    pub elements: Vec<ElementDescriptor>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareServiceDescriptor {
    pub service_id: ServiceId,
    pub provider_id: ProviderId,
    pub label: String,
    #[serde(default)]
    pub elements: Vec<ElementDescriptor>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Local,
    Remote,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDescriptor {
    pub node_id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    pub descriptor_revision: DescriptorRevision,
    #[serde(default)]
    pub devices: Vec<DeviceDescriptor>,
    #[serde(default)]
    pub services: Vec<SoftwareServiceDescriptor>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DescriptorError {
    #[error("недопустимый port kind: {0}")]
    InvalidPortKind(String),
    #[error("numeric range minimum выше maximum")]
    InvalidRange,
    #[error("port kind/direction/semantics противоречат друг другу")]
    PortSemanticsMismatch,
    #[error("повторяющийся ID в descriptor: {0}")]
    DuplicateId(String),
    #[error("identity evidence confidence должна быть в диапазоне 0..=1")]
    InvalidConfidence,
    #[error("rate hints должны быть положительными и maximum не ниже preferred")]
    InvalidRate,
    #[error(transparent)]
    Schema(#[from] crate::schema::SchemaError),
}

impl PortDescriptor {
    pub fn validate(&self) -> Result<(), DescriptorError> {
        self.compatible_versions.validate()?;
        if self.range.as_ref().is_some_and(|range| {
            !range.minimum.is_finite()
                || !range.maximum.is_finite()
                || range.minimum > range.maximum
                || range.center.is_some_and(|center| {
                    !center.is_finite() || center < range.minimum || center > range.maximum
                })
                || range
                    .precision
                    .is_some_and(|precision| !precision.is_finite() || precision <= 0.0)
        }) {
            return Err(DescriptorError::InvalidRange);
        }
        if self.rate.as_ref().is_some_and(|rate| {
            rate.preferred_hz
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
                || rate
                    .maximum_hz
                    .is_some_and(|value| !value.is_finite() || value <= 0.0)
                || rate.maximum_latency_ms == Some(0)
                || rate
                    .preferred_hz
                    .zip(rate.maximum_hz)
                    .is_some_and(|(preferred, maximum)| preferred > maximum)
        }) {
            return Err(DescriptorError::InvalidRate);
        }
        let expected = match self.kind.as_str() {
            PortKind::EVENT_OUT => (PortDirection::Output, PortSemantics::Event),
            PortKind::VALUE_OUT => (PortDirection::Output, PortSemantics::Value),
            PortKind::STATE_OUT => (PortDirection::Output, PortSemantics::State),
            PortKind::COMMAND_IN => (PortDirection::Input, PortSemantics::Command),
            PortKind::VALUE_IN => (PortDirection::Input, PortSemantics::Value),
            PortKind::FRAME_IN => (PortDirection::Input, PortSemantics::Frame),
            PortKind::HAPTIC_IN => (PortDirection::Input, PortSemantics::Haptic),
            PortKind::STREAM_OUT => (PortDirection::Output, PortSemantics::Stream),
            PortKind::STREAM_IN => (PortDirection::Input, PortSemantics::Stream),
            _ => return Ok(()),
        };
        if (self.direction, self.semantics) != expected {
            return Err(DescriptorError::PortSemanticsMismatch);
        }
        Ok(())
    }
}

impl NodeDescriptor {
    pub fn validate(&self) -> Result<(), DescriptorError> {
        let mut identities = BTreeSet::new();
        let mut service_ids = BTreeSet::new();
        for device in &self.devices {
            if !identities.insert(device.identity.as_str()) {
                return Err(DescriptorError::DuplicateId(device.identity.to_string()));
            }
            if device
                .evidence
                .iter()
                .any(|evidence| !(0.0..=1.0).contains(&evidence.confidence))
            {
                return Err(DescriptorError::InvalidConfidence);
            }
            validate_elements(&device.elements)?;
        }
        for service in &self.services {
            if !service_ids.insert(service.service_id.as_str()) {
                return Err(DescriptorError::DuplicateId(service.service_id.to_string()));
            }
            validate_elements(&service.elements)?;
        }
        Ok(())
    }
}

fn validate_elements(elements: &[ElementDescriptor]) -> Result<(), DescriptorError> {
    let mut element_ids = BTreeSet::new();
    for element in elements {
        if !element_ids.insert(element.element_id.as_str()) {
            return Err(DescriptorError::DuplicateId(element.element_id.to_string()));
        }
        let mut capability_ids = BTreeSet::new();
        for capability in &element.capabilities {
            if !capability_ids.insert(capability.capability_id.as_str()) {
                return Err(DescriptorError::DuplicateId(
                    capability.capability_id.to_string(),
                ));
            }
            let mut port_ids = BTreeSet::new();
            for port in &capability.ports {
                if !port_ids.insert(port.port_id.as_str()) {
                    return Err(DescriptorError::DuplicateId(port.port_id.to_string()));
                }
                port.validate()?;
            }
        }
    }
    Ok(())
}

pub type StateFields = BTreeMap<String, Value>;
