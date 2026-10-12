// SPDX-License-Identifier: GPL-3.0-or-later
//! Порты application layer без зависимостей от Tauri и реализаций ОС.

use std::path::PathBuf;

use deckweave_domain::{
    DeploymentMode, LifecycleState,
    ids::{CredentialLeaseId, GrantRevision},
    security::{CredentialLease, CredentialRequest, PermissionGrant, PermissionRequest},
};
use serde::Serialize;
use thiserror::Error;

pub mod api;
pub use api::*;
mod apply;
pub use apply::*;
mod authorization;
pub use authorization::DenyAllAuthorizationStore;
mod scoped_authorization;
pub use scoped_authorization::ScopedAuthorizationStore;
mod gateway;
pub use gateway::{ApplicationGateway, HostStatusSource, InMemoryApiAuthorizer};
mod diagnostics;
pub mod plugin_data;
pub mod plugin_services;
pub use diagnostics::*;
// Compatibility export while downstream SDK users migrate to deckweave-platform.
pub use deckweave_platform::*;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuthorizationError {
    #[error("разрешение отсутствует или отозвано")]
    Denied,
    #[error("authorization store недоступен: {0}")]
    Unavailable(String),
}

/// Grants хранятся вне переносимого profile и проверяются при каждом
/// privileged dispatch/lease, а не только при компиляции workspace.
pub trait AuthorizationStore: Send + Sync {
    fn revision(&self) -> Result<GrantRevision, AuthorizationError>;
    fn authorize(&self, request: &PermissionRequest)
    -> Result<PermissionGrant, AuthorizationError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CredentialError {
    #[error("credential reference не сопоставлен локальному secret")]
    Unresolved,
    #[error("credential lease отсутствует, истёк или отозван")]
    InvalidLease,
    #[error("credential broker недоступен: {0}")]
    Unavailable(String),
}

/// Broker выдаёт только scoped opaque lease. Конкретный adapter может
/// выполнить операцию с secret, не раскрывая plaintext вызывающему plugin.
pub trait CredentialBroker: Send + Sync {
    /// Read-only preflight; it never exposes a secret or implies a runtime lease.
    fn check_reference(&self, _request: &CredentialRequest) -> Result<(), CredentialError> {
        Err(CredentialError::Unresolved)
    }
    fn acquire(&self, request: &CredentialRequest) -> Result<CredentialLease, CredentialError>;
    fn revoke(&self, lease_id: &CredentialLeaseId) -> Result<(), CredentialError>;
}

pub struct UnresolvedCredentialBroker;
impl CredentialBroker for UnresolvedCredentialBroker {
    fn acquire(&self, _request: &CredentialRequest) -> Result<CredentialLease, CredentialError> {
        Err(CredentialError::Unresolved)
    }
    fn revoke(&self, _lease_id: &CredentialLeaseId) -> Result<(), CredentialError> {
        Err(CredentialError::InvalidLease)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSnapshot {
    pub lifecycle: LifecycleState,
    pub mode: DeploymentMode,
    pub data_root: PathBuf,
    pub version: String,
    pub notice: Option<String>,
}

/// Canonical transport-independent contract used by Desktop, CLI and future
/// local HTTP/MCP adapters. Authorization stays inside the implementation.
pub trait ApplicationService: Send + Sync {
    fn snapshot(&self) -> HostSnapshot;
    fn open_mutation_window(&self, context: &RequestContext) -> Result<MutationWindow, ApiError>;
    fn query(&self, context: &RequestContext, query: AppQuery) -> Result<AppQueryResult, ApiError>;
    fn command(
        &self,
        context: &RequestContext,
        envelope: CommandEnvelope,
    ) -> Result<AppCommandResult, ApiError>;
}
