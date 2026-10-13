// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeWindow {
    pub token: u64,
    pub incarnation: u64,
    pub process_id: u32,
    pub process_started_at: Option<u64>,
    pub executable: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WindowReference {
    pub generation: u64,
    pub reference: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WindowSelector {
    pub executable: String,
    #[serde(default)]
    pub title_contains: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WindowAvailability {
    Available,
    NoForeground,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveWindowState {
    pub availability: WindowAvailability,
    pub generation: u64,
    pub window_ref: Option<String>,
    pub process_id: Option<u32>,
    pub executable: Option<String>,
    pub title: String,
    pub fresh: bool,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowCatalogItem {
    pub window_ref: String,
    pub process_id: u32,
    pub executable: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WindowCatalogRequest {
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowCatalog {
    pub generation: u64,
    pub windows: Vec<WindowCatalogItem>,
    pub total: u32,
    pub next_offset: Option<u32>,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowFailure {
    UnsupportedOperatingSystem,
    InvalidInput,
    NoMatchingWindow,
    StaleReference,
    ForegroundDenied,
    PermissionDenied,
    ResourceBusy,
    OsApiUnavailable,
    WorkerStopped,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectKnowledge {
    NotApplied,
    PossiblyApplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowCommandError {
    pub failure: WindowFailure,
    pub effect: EffectKnowledge,
}

impl WindowCommandError {
    pub(crate) fn before_effect(failure: WindowFailure) -> Self {
        Self {
            failure,
            effect: EffectKnowledge::NotApplied,
        }
    }

    pub(crate) fn after_attempt(failure: WindowFailure) -> Self {
        Self {
            failure,
            effect: EffectKnowledge::PossiblyApplied,
        }
    }
}
