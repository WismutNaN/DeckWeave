// SPDX-License-Identifier: GPL-3.0-or-later
use crate::audio::AudioFailure;
use crate::window::WindowFailure;
use deckweave_protocol::v1::{ErrorCategory, StructuredError};
use std::{collections::HashMap, error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFailure {
    InvalidInput,
    DeadlineExceeded,
    UnsupportedOperatingSystem,
    UnsupportedBuild,
    PermissionDenied,
    ResourceBusy,
    NoDefaultEndpoint,
    NoMediaSession,
    InputBlocked,
    PartialInput,
    NoMatchingWindow,
    StaleWindowReference,
    ForegroundDenied,
    LaunchRejected,
    OsApiUnavailable,
    TransientRebind,
}

impl PlatformFailure {
    #[must_use]
    pub fn controlled_adapter() -> Self {
        if cfg!(windows) {
            Self::OsApiUnavailable
        } else {
            Self::UnsupportedOperatingSystem
        }
    }

    #[must_use]
    pub fn structured(self) -> StructuredError {
        let (category, code) = match self {
            Self::InvalidInput => (ErrorCategory::InvalidInput, "invalid_input"),
            Self::DeadlineExceeded => (ErrorCategory::DeadlineExceeded, "deadline_exceeded"),
            Self::UnsupportedOperatingSystem => {
                (ErrorCategory::Unsupported, "unsupported_operating_system")
            }
            Self::UnsupportedBuild => (ErrorCategory::Unsupported, "unsupported_build"),
            Self::PermissionDenied => (ErrorCategory::PermissionDenied, "permission_denied"),
            Self::ResourceBusy => (ErrorCategory::ResourceBusy, "resource_busy"),
            Self::NoDefaultEndpoint => (ErrorCategory::Unavailable, "no_default_endpoint"),
            Self::InputBlocked => (ErrorCategory::Unavailable, "input_blocked"),
            Self::PartialInput => (ErrorCategory::Unavailable, "partial_or_unknown"),
            Self::NoMediaSession => (ErrorCategory::Unavailable, "no_media_session"),
            Self::NoMatchingWindow => (ErrorCategory::Unavailable, "no_matching_window"),
            Self::StaleWindowReference => (ErrorCategory::Unavailable, "stale_window_reference"),
            Self::ForegroundDenied => (ErrorCategory::PermissionDenied, "foreground_denied"),
            Self::LaunchRejected => (ErrorCategory::PermanentExternal, "launch_rejected"),
            Self::OsApiUnavailable => (ErrorCategory::Unavailable, "os_api_unavailable"),
            Self::TransientRebind => (ErrorCategory::TransientExternal, "transient_rebind"),
        };
        StructuredError {
            category: category as i32,
            code: code.into(),
            message_key: format!("platform.windows.error.{code}"),
            details: HashMap::new(),
            diagnostic_attachment_id: None,
        }
    }
}

impl From<WindowFailure> for PlatformFailure {
    fn from(failure: WindowFailure) -> Self {
        match failure {
            WindowFailure::UnsupportedOperatingSystem => Self::UnsupportedOperatingSystem,
            WindowFailure::DeadlineExceeded => Self::DeadlineExceeded,
            WindowFailure::InvalidInput => Self::InvalidInput,
            WindowFailure::NoMatchingWindow => Self::NoMatchingWindow,
            WindowFailure::StaleReference => Self::StaleWindowReference,
            WindowFailure::ForegroundDenied => Self::ForegroundDenied,
            WindowFailure::PermissionDenied => Self::PermissionDenied,
            WindowFailure::ResourceBusy => Self::ResourceBusy,
            WindowFailure::OsApiUnavailable => Self::OsApiUnavailable,
            WindowFailure::WorkerStopped => Self::TransientRebind,
        }
    }
}

impl From<AudioFailure> for PlatformFailure {
    fn from(failure: AudioFailure) -> Self {
        match failure {
            AudioFailure::UnsupportedOperatingSystem => Self::UnsupportedOperatingSystem,
            AudioFailure::DeadlineExceeded => Self::DeadlineExceeded,
            AudioFailure::InvalidInput => Self::InvalidInput,
            AudioFailure::PermissionDenied => Self::PermissionDenied,
            AudioFailure::NoDefaultEndpoint => Self::NoDefaultEndpoint,
            AudioFailure::ResourceBusy => Self::ResourceBusy,
            AudioFailure::TransientRebind | AudioFailure::WorkerStopped => Self::TransientRebind,
            AudioFailure::OsApiUnavailable => Self::OsApiUnavailable,
        }
    }
}

impl fmt::Display for PlatformFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for PlatformFailure {}
