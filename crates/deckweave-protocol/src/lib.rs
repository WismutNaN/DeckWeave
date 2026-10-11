// SPDX-License-Identifier: MIT OR Apache-2.0
//! Permissive wire contract. This crate never imports GPL application crates.

use prost::Message;
use std::collections::BTreeMap;
use thiserror::Error;

pub mod framing;

pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/deckweave.v1.rs"));
}

/// Rust document types generated from the same JSON Schemas as the TypeScript
/// declarations. Extensible objects use schema-valued `additionalProperties`
/// so unknown optional fields survive a typed JSON round-trip.
pub mod documents {
    pub mod plugin_manifest {
        typify::import_types!(schema = "../../protocol/schema/plugin-manifest.schema.json");
    }
    pub mod settings_document {
        typify::import_types!(schema = "../../protocol/schema/settings-document.schema.json");
    }
    pub mod workspace_profile {
        typify::import_types!(schema = "../../protocol/schema/workspace-profile.schema.json");
    }
}

/// Bytes held by a transparent relay. A typed Prost decode/re-encode is not
/// transparent because unknown fields are not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueProtocolFrame(Vec<u8>);

impl OpaqueProtocolFrame {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("control frame превышает negotiated limit: {actual} > {limit}")]
    TooLarge { actual: usize, limit: usize },
    #[error("некорректный Protocol Buffer: {0}")]
    Decode(#[from] prost::DecodeError),
}

pub fn decode_envelope(bytes: &[u8], limit: usize) -> Result<v1::ProtocolEnvelope, FrameError> {
    if bytes.len() > limit {
        return Err(FrameError::TooLarge {
            actual: bytes.len(),
            limit,
        });
    }
    Ok(v1::ProtocolEnvelope::decode(bytes)?)
}

#[derive(Debug, Clone)]
pub struct HostProtocolSupport {
    pub protocol: v1::ProtocolVersionRange,
    pub features: BTreeMap<String, u32>,
    pub schemas: BTreeMap<String, v1::ProtocolVersionRange>,
    pub limits: v1::ProtocolLimits,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NegotiationError {
    #[error("отсутствует или некорректен protocol range")]
    InvalidProtocolRange,
    #[error("нет общей версии control protocol")]
    ProtocolMismatch,
    #[error("обязательная feature не поддерживается: {0}")]
    RequiredFeature(String),
    #[error("обязательная schema не поддерживается: {0}")]
    RequiredSchema(String),
    #[error("negotiated limits должны быть больше нуля")]
    InvalidLimits,
}

pub fn negotiate_hello(
    hello: &v1::Hello,
    support: &HostProtocolSupport,
    plugin_session_id: String,
) -> Result<v1::HelloAck, NegotiationError> {
    let plugin_protocol = hello
        .protocol
        .as_ref()
        .ok_or(NegotiationError::InvalidProtocolRange)?;
    let selected_protocol = select_version(plugin_protocol, &support.protocol)
        .ok_or(NegotiationError::ProtocolMismatch)?;

    let mut features = Vec::new();
    for requirement in &hello.features {
        let selected = support
            .features
            .get(&requirement.id)
            .copied()
            .filter(|version| {
                *version >= requirement.minimum_version && *version <= requirement.maximum_version
            });
        match selected {
            Some(version) => features.push(v1::NegotiatedFeature {
                id: requirement.id.clone(),
                version,
            }),
            None if requirement.required => {
                return Err(NegotiationError::RequiredFeature(requirement.id.clone()));
            }
            None => {}
        }
    }

    let mut schemas = Vec::new();
    for requirement in &hello.schemas {
        let requested = v1::ProtocolVersionRange {
            minimum: Some(v1::ProtocolVersion {
                major: requirement.minimum_major,
                minor: requirement.minimum_minor,
            }),
            maximum: Some(v1::ProtocolVersion {
                major: requirement.maximum_major,
                minor: requirement.maximum_minor,
            }),
        };
        let selected = support
            .schemas
            .get(&requirement.id)
            .and_then(|available| select_version(&requested, available));
        match selected {
            Some(version) => schemas.push(v1::NegotiatedSchema {
                schema: Some(v1::SchemaReference {
                    id: requirement.id.clone(),
                    major: version.major,
                    minor: version.minor,
                }),
            }),
            None if requirement.required => {
                return Err(NegotiationError::RequiredSchema(requirement.id.clone()));
            }
            None => {}
        }
    }

    let limits = negotiate_limits(
        hello
            .limits
            .as_ref()
            .ok_or(NegotiationError::InvalidLimits)?,
        &support.limits,
    )?;
    Ok(v1::HelloAck {
        selected_protocol: Some(selected_protocol),
        plugin_session_id,
        features,
        schemas,
        limits: Some(limits),
    })
}

fn version_key(version: &v1::ProtocolVersion) -> (u32, u32) {
    (version.major, version.minor)
}

fn select_version(
    left: &v1::ProtocolVersionRange,
    right: &v1::ProtocolVersionRange,
) -> Option<v1::ProtocolVersion> {
    let left_min = left.minimum.as_ref()?;
    let left_max = left.maximum.as_ref()?;
    let right_min = right.minimum.as_ref()?;
    let right_max = right.maximum.as_ref()?;
    if version_key(left_min) > version_key(left_max)
        || version_key(right_min) > version_key(right_max)
    {
        return None;
    }
    let lower = std::cmp::max(version_key(left_min), version_key(right_min));
    let upper = std::cmp::min(version_key(left_max), version_key(right_max));
    (lower <= upper).then_some(v1::ProtocolVersion {
        major: upper.0,
        minor: upper.1,
    })
}

fn negotiate_limits(
    requested: &v1::ProtocolLimits,
    policy: &v1::ProtocolLimits,
) -> Result<v1::ProtocolLimits, NegotiationError> {
    let limits = v1::ProtocolLimits {
        maximum_message_bytes: requested
            .maximum_message_bytes
            .min(policy.maximum_message_bytes),
        maximum_chunk_bytes: requested
            .maximum_chunk_bytes
            .min(policy.maximum_chunk_bytes),
        maximum_concurrent_jobs: requested
            .maximum_concurrent_jobs
            .min(policy.maximum_concurrent_jobs),
        maximum_artifact_bytes: requested
            .maximum_artifact_bytes
            .min(policy.maximum_artifact_bytes),
        maximum_events_per_second: requested
            .maximum_events_per_second
            .min(policy.maximum_events_per_second),
    };
    if limits.maximum_message_bytes == 0
        || limits.maximum_chunk_bytes == 0
        || limits.maximum_concurrent_jobs == 0
        || limits.maximum_artifact_bytes == 0
        || limits.maximum_events_per_second == 0
    {
        return Err(NegotiationError::InvalidLimits);
    }
    Ok(limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use serde_json::Value;

    fn version_range(minor_min: u32, minor_max: u32) -> v1::ProtocolVersionRange {
        v1::ProtocolVersionRange {
            minimum: Some(v1::ProtocolVersion {
                major: 1,
                minor: minor_min,
            }),
            maximum: Some(v1::ProtocolVersion {
                major: 1,
                minor: minor_max,
            }),
        }
    }

    fn limits(value: u64) -> v1::ProtocolLimits {
        v1::ProtocolLimits {
            maximum_message_bytes: value,
            maximum_chunk_bytes: value,
            maximum_concurrent_jobs: u32::try_from(value).unwrap_or(u32::MAX),
            maximum_artifact_bytes: value,
            maximum_events_per_second: u32::try_from(value).unwrap_or(u32::MAX),
        }
    }

    #[test]
    fn required_features_and_limits_are_negotiated() -> Result<(), Box<dyn std::error::Error>> {
        let hello = v1::Hello {
            protocol: Some(version_range(0, 3)),
            package_id: "org.deckweave.fixture".into(),
            runtime_group: "control".into(),
            one_time_token: vec![1, 2, 3],
            features: vec![v1::FeatureRequirement {
                id: "org.deckweave.jobs".into(),
                minimum_version: 1,
                maximum_version: 2,
                required: true,
            }],
            schemas: Vec::new(),
            limits: Some(limits(128)),
        };
        let support = HostProtocolSupport {
            protocol: version_range(1, 2),
            features: BTreeMap::from([("org.deckweave.jobs".into(), 2)]),
            schemas: BTreeMap::new(),
            limits: limits(64),
        };
        let reply = negotiate_hello(&hello, &support, "session-1".into())?;
        assert_eq!(reply.selected_protocol.as_ref().map(|v| v.minor), Some(2));
        assert_eq!(reply.features.first().map(|f| f.version), Some(2));
        assert_eq!(reply.limits.map(|l| l.maximum_message_bytes), Some(64));
        Ok(())
    }

    #[test]
    fn typed_reencode_drops_unknown_field_but_opaque_relay_preserves_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let message = v1::ProtocolEnvelope {
            wire_version: Some(v1::ProtocolVersion { major: 1, minor: 0 }),
            session_generation: 1,
            message_id: 7,
            trace: None,
            body: Some(v1::protocol_envelope::Body::Heartbeat(v1::Heartbeat {
                host_ticks: 10,
                active_jobs: 0,
            })),
        };
        let mut bytes = message.encode_to_vec();
        // Unknown field 127, varint value 1.
        bytes.extend_from_slice(&[0xf8, 0x07, 0x01]);
        let opaque = OpaqueProtocolFrame::new(bytes.clone());
        let decoded = decode_envelope(&bytes, 1024)?;
        assert_ne!(decoded.encode_to_vec(), bytes);
        assert_eq!(opaque.as_bytes(), bytes);
        Ok(())
    }

    #[test]
    fn unknown_enum_number_and_optional_presence_are_not_collapsed()
    -> Result<(), Box<dyn std::error::Error>> {
        let error = v1::StructuredError {
            category: 31_337,
            code: "future".into(),
            message_key: "future".into(),
            details: Default::default(),
            diagnostic_attachment_id: None,
        };
        let decoded = v1::StructuredError::decode(error.encode_to_vec().as_slice())?;
        assert_eq!(decoded.category, 31_337);

        let base = v1::FramePacket {
            target_port_id: "display".into(),
            activation_epoch: 1,
            descriptor_revision: 1,
            frame_id: 1,
            base_frame_id: None,
            chunk_index: 0,
            chunk_count: 1,
            chunk: None,
        };
        let mut present = base.clone();
        present.base_frame_id = Some(0);
        assert_ne!(base.encode_to_vec(), present.encode_to_vec());
        assert_eq!(
            v1::FramePacket::decode(present.encode_to_vec().as_slice())?.base_frame_id,
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn rust_reads_the_shared_wire_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let hex = include_str!("../../../protocol/fixtures/wire/heartbeat-envelope.hex").trim();
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?;
        let decoded = decode_envelope(&bytes, 1024)?;
        assert_eq!(decoded.session_generation, 4);
        assert_eq!(decoded.message_id, 9);
        assert_eq!(decoded.encode_to_vec(), bytes);
        let Some(v1::protocol_envelope::Body::Heartbeat(heartbeat)) = decoded.body else {
            return Err("fixture is not a heartbeat".into());
        };
        assert_eq!(heartbeat.host_ticks, 12);
        assert_eq!(heartbeat.active_jobs, 1);
        Ok(())
    }

    #[test]
    fn generated_rust_documents_preserve_unknown_optional_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let source =
            include_str!("../../../protocol/fixtures/documents/plugin-manifest.future.json");
        let document: documents::plugin_manifest::PluginManifest = serde_json::from_str(source)?;
        let round_trip: Value = serde_json::to_value(document)?;
        assert_eq!(
            round_trip["futureOptionalField"]["mustSurvive"],
            Value::Bool(true)
        );
        assert_eq!(
            round_trip["source"]["futureSourceProof"]["algorithm"],
            Value::String("future-v2".into())
        );

        let mut unsafe_document: Value = serde_json::from_str(source)?;
        unsafe_document["artifacts"][0]["entrypoint"] = Value::String("../plugin.exe".into());
        assert!(
            serde_json::from_value::<documents::plugin_manifest::PluginManifest>(unsafe_document)
                .is_err()
        );
        Ok(())
    }
}
