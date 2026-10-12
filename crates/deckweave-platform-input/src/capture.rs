// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{PrivilegeAvailability, PrivilegeStatus};
use deckweave_domain::{
    capability::IdentityEvidence,
    ids::{DeviceIdentity, ProviderId},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputGranularity {
    SystemAggregate,
    OsLogicalDevice,
    PhysicalDevice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSourceDescriptor {
    pub provider_id: ProviderId,
    pub label: String,
    pub granularity: InputGranularity,
    pub identity: Option<DeviceIdentity>,
    #[serde(default)]
    pub evidence: Vec<IdentityEvidence>,
    pub can_observe: bool,
    pub can_intercept: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputEmitterDescriptor {
    pub provider_id: ProviderId,
    pub label: String,
    pub virtual_identity: DeviceIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    Observe,
    Intercept,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePolicy {
    pub mode: CaptureMode,
    pub pass_through: bool,
    pub fail_open: bool,
}

impl CapturePolicy {
    #[must_use]
    pub const fn observe() -> Self {
        Self {
            mode: CaptureMode::Observe,
            pass_through: true,
            fail_open: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InputContractError {
    #[error("input source label is empty")]
    EmptyLabel,
    #[error("physical-device granularity requires a stable identity and evidence")]
    MissingPhysicalEvidence,
    #[error("aggregate input must not claim a physical-device identity")]
    AggregateClaimsPhysicalIdentity,
    #[error("requested capture mode is unsupported")]
    UnsupportedMode,
    #[error("observe mode must preserve pass-through")]
    ObserveMustPassThrough,
    #[error("intercept mode requires fail-open recovery")]
    InterceptMustFailOpen,
    #[error("degraded or unavailable privilege status requires a stable reason code")]
    MissingPrivilegeReason,
}

pub fn validate_source(source: &InputSourceDescriptor) -> Result<(), InputContractError> {
    if source.label.trim().is_empty() {
        return Err(InputContractError::EmptyLabel);
    }
    match source.granularity {
        InputGranularity::PhysicalDevice
            if source.identity.is_none() || source.evidence.is_empty() =>
        {
            Err(InputContractError::MissingPhysicalEvidence)
        }
        InputGranularity::SystemAggregate if source.identity.is_some() => {
            Err(InputContractError::AggregateClaimsPhysicalIdentity)
        }
        _ => Ok(()),
    }
}

pub fn validate_capture(
    source: &InputSourceDescriptor,
    policy: CapturePolicy,
) -> Result<(), InputContractError> {
    validate_source(source)?;
    match policy.mode {
        CaptureMode::Observe if !source.can_observe => Err(InputContractError::UnsupportedMode),
        CaptureMode::Observe if !policy.pass_through => {
            Err(InputContractError::ObserveMustPassThrough)
        }
        CaptureMode::Intercept if !source.can_intercept => Err(InputContractError::UnsupportedMode),
        CaptureMode::Intercept if !policy.fail_open => {
            Err(InputContractError::InterceptMustFailOpen)
        }
        _ => Ok(()),
    }
}

pub fn validate_privilege(status: &PrivilegeStatus) -> Result<(), InputContractError> {
    if status.availability != PrivilegeAvailability::Available && status.reason.is_none() {
        return Err(InputContractError::MissingPrivilegeReason);
    }
    Ok(())
}
