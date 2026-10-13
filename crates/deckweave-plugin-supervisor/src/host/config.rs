// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime limits and errors shared by the production coordinator.

use crate::{process::ProcessError, supervisor::SupervisorError};
use deckweave_application::AuthorizationError;
use deckweave_protocol::{
    FEATURE_INVOCATION_ENDPOINT_PATH, FEATURE_VALUE_WRITE_ENDPOINT_PATH, HostProtocolSupport,
    v1::{ProtocolLimits, ProtocolVersion, ProtocolVersionRange},
};
use deckweave_registry::RegistryError;
use deckweave_workspace::{ApplyRepositoryError, DraftRepositoryError};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct HostRuntimeConfig {
    pub user_process_launcher: Option<std::sync::Arc<dyn deckweave_platform::UserProcessLauncher>>,
    pub locale: deckweave_domain::localization::LocaleContext,
    pub process: crate::process::ProcessConfig,
    pub restart_policy: crate::supervisor::RestartPolicy,
    pub protocol_support: HostProtocolSupport,
    pub handshake_timeout: Duration,
    pub monitor_interval: Duration,
    pub stop_timeout: Duration,
    pub shutdown_timeout: Duration,
}

impl Default for HostRuntimeConfig {
    fn default() -> Self {
        Self {
            user_process_launcher: None,
            locale: Default::default(),
            process: crate::process::ProcessConfig::default(),
            restart_policy: crate::supervisor::RestartPolicy::default(),
            protocol_support: HostProtocolSupport {
                protocol: ProtocolVersionRange {
                    minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
                    maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
                },
                features: std::collections::BTreeMap::from([
                    (deckweave_protocol::FEATURE_ACTIVATION_COMMIT.into(), 1),
                    (deckweave_protocol::FEATURE_CONFIGURED_INSTANCES.into(), 1),
                    (deckweave_protocol::FEATURE_CONFIGURED_DISPATCH.into(), 1),
                    (deckweave_protocol::FEATURE_LOCALE_CONTEXT.into(), 1),
                    (deckweave_protocol::FEATURE_OWNER_CLEANUP.into(), 1),
                    (FEATURE_INVOCATION_ENDPOINT_PATH.into(), 1),
                    (FEATURE_VALUE_WRITE_ENDPOINT_PATH.into(), 1),
                ]),
                schemas: Default::default(),
                limits: ProtocolLimits {
                    maximum_message_bytes: 1024 * 1024,
                    maximum_chunk_bytes: 64 * 1024,
                    maximum_concurrent_jobs: 16,
                    maximum_artifact_bytes: 128 * 1024 * 1024,
                    maximum_events_per_second: 1_000,
                },
            },
            handshake_timeout: Duration::from_secs(5),
            monitor_interval: Duration::from_millis(50),
            stop_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(4),
        }
    }
}

#[derive(Debug, Error)]
pub enum HostRuntimeError {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("runtime protocol error: {0}")]
    Protocol(String),
    #[error("runtime state error: {0}")]
    State(String),
    #[error("runtime shutdown error: {0}")]
    Shutdown(String),
    #[error("required package is unavailable: {0}")]
    Package(String),
}

impl From<DraftRepositoryError> for HostRuntimeError {
    fn from(error: DraftRepositoryError) -> Self {
        Self::State(error.to_string())
    }
}

impl From<ApplyRepositoryError> for HostRuntimeError {
    fn from(error: ApplyRepositoryError) -> Self {
        Self::State(error.to_string())
    }
}

impl From<AuthorizationError> for HostRuntimeError {
    fn from(error: AuthorizationError) -> Self {
        Self::State(error.to_string())
    }
}
