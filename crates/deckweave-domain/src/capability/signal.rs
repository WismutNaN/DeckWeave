// SPDX-License-Identifier: GPL-3.0-or-later
//! Normalized signal ingress after authenticated runtime resolution.

use crate::{
    endpoint::{EndpointTargetRef, RuntimeEndpointRef},
    ids::{
        ActivationEpoch, CausationId, ClockDomainId, CorrelationId, HostBootId, IngressOrdinal,
        SessionGeneration, TraceId,
    },
    schema::ExtensionMap,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalQuality {
    Fresh,
    Stale,
    Estimated,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalOrigin {
    Physical,
    Synthetic,
    Remote,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTimestamp {
    pub ticks: u64,
    pub frequency_hz: u64,
    pub clock_domain_id: ClockDomainId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostIngress {
    pub received_ticks: u64,
    pub ingress_ordinal: IngressOrdinal,
    pub host_boot_id: HostBootId,
    pub activation_epoch: ActivationEpoch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub correlation_id: Option<CorrelationId>,
    pub causation_id: Option<CausationId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalEnvelope {
    pub source: RuntimeEndpointRef,
    pub device_session_generation: Option<SessionGeneration>,
    pub sequence: u64,
    pub captured_at: SourceTimestamp,
    pub host_ingress: Option<HostIngress>,
    pub wall_clock_rfc3339: Option<String>,
    pub quality: SignalQuality,
    pub origin: SignalOrigin,
    pub trace: TraceContext,
    pub payload: Value,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SignalValidationError {
    #[error("source clock frequency и sequence должны быть больше нуля")]
    InvalidClock,
    #[error("device signal должен содержать device session generation")]
    MissingDeviceGeneration,
    #[error("service signal не может содержать device session generation")]
    UnexpectedDeviceGeneration,
}

impl SignalEnvelope {
    pub fn validate(&self) -> Result<(), SignalValidationError> {
        if self.sequence == 0 || self.captured_at.frequency_hz == 0 {
            return Err(SignalValidationError::InvalidClock);
        }
        match (&self.source.stable.target, self.device_session_generation) {
            (EndpointTargetRef::Device { .. }, None) => {
                Err(SignalValidationError::MissingDeviceGeneration)
            }
            (EndpointTargetRef::Service { .. }, Some(_)) => {
                Err(SignalValidationError::UnexpectedDeviceGeneration)
            }
            _ => Ok(()),
        }
    }
}
