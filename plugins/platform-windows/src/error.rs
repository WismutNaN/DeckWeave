// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_protocol::v1::{ErrorCategory, StructuredError};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFailure {
    UnsupportedOperatingSystem,
    UnsupportedBuild,
    PermissionDenied,
    NoDefaultEndpoint,
    NoMediaSession,
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
            Self::UnsupportedOperatingSystem => {
                (ErrorCategory::Unsupported, "unsupported_operating_system")
            }
            Self::UnsupportedBuild => (ErrorCategory::Unsupported, "unsupported_build"),
            Self::PermissionDenied => (ErrorCategory::PermissionDenied, "permission_denied"),
            Self::NoDefaultEndpoint => (ErrorCategory::Unavailable, "no_default_endpoint"),
            Self::NoMediaSession => (ErrorCategory::Unavailable, "no_media_session"),
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
