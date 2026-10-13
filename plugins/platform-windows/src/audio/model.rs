// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCommand {
    SetVolume(f64),
    ChangeVolume(f64),
    SetMute(bool),
    ToggleMute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAvailability {
    Available,
    Unavailable,
    Rebind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioState {
    pub endpoint_ref: String,
    pub scalar: f64,
    pub muted: bool,
    pub availability: AudioAvailability,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct AudioReading {
    pub endpoint_ref: String,
    pub scalar: f64,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFailure {
    UnsupportedOperatingSystem,
    InvalidInput,
    PermissionDenied,
    NoDefaultEndpoint,
    ResourceBusy,
    TransientRebind,
    OsApiUnavailable,
    WorkerStopped,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKnowledge {
    NotApplied,
    PossiblyApplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("audio command failed: {failure:?} ({effect:?})")]
pub struct AudioCommandError {
    pub failure: AudioFailure,
    pub effect: EffectKnowledge,
}

impl AudioCommandError {
    pub(super) const fn before_effect(failure: AudioFailure) -> Self {
        Self {
            failure,
            effect: EffectKnowledge::NotApplied,
        }
    }

    pub(super) const fn after_effect(failure: AudioFailure) -> Self {
        Self {
            failure,
            effect: EffectKnowledge::PossiblyApplied,
        }
    }
}
