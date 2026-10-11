// SPDX-License-Identifier: GPL-3.0-or-later
//! Порты application layer без зависимостей от Tauri и реализаций ОС.

use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
};

use deckweave_domain::{
    DeploymentMode, LifecycleState,
    ids::{CredentialLeaseId, GrantRevision},
    security::{CredentialLease, CredentialRequest, PermissionGrant, PermissionRequest},
};
use serde::Serialize;
use thiserror::Error;

pub mod api;
pub use api::*;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("{0}")]
    Unavailable(String),
    #[error("Каталог данных уже используется: {0}")]
    RootBusy(PathBuf),
    #[error("Другой DeckWeave уже работает в этой пользовательской сессии")]
    SessionBusy,
    #[error("В portable-режиме автозапуск отключён")]
    PortableAutostart,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Ресурсы владения освобождаются при Drop/аварийном завершении процесса.
pub trait HostLease: Send + Sync {}

/// Только bootstrap-сервисы; пользовательские OS actions будут плагинами.
pub trait HostPlatform: Send + Sync {
    fn data_root(&self) -> &std::path::Path;
    fn mode(&self) -> DeploymentMode;
    fn acquire(&self) -> Result<Box<dyn HostLease>, PlatformError>;
    fn autostart_enabled(&self) -> Result<bool, PlatformError>;
    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError>;
}

/// OS-owned lifetime boundary for a plugin process tree. Dropping the guard
/// must not leave descendants running after the runtime group is stopped.
pub trait ChildProcessGuard: Send {
    fn terminate_tree(&mut self) -> Result<(), PlatformError>;
}

/// Configures process-tree isolation before spawn and attaches the native
/// lifetime object immediately afterwards.
pub trait ChildProcessPolicy: Send + Sync {
    fn configure(&self, command: &mut Command) -> Result<(), PlatformError>;
    fn attach(&self, child: &mut Child) -> Result<Box<dyn ChildProcessGuard>, PlatformError>;
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("durable storage недоступно: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Minimal durability port used by JSON source-of-truth stores. The adapter
/// must create the replacement on the same filesystem, flush it before
/// publication and replace `target` atomically for the current OS.
pub trait AtomicFileStore: Send + Sync {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), StorageError>;
}

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
    fn acquire(&self, request: &CredentialRequest) -> Result<CredentialLease, CredentialError>;
    fn revoke(&self, lease_id: &CredentialLeaseId) -> Result<(), CredentialError>;
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
    fn query(&self, context: &RequestContext, query: AppQuery) -> Result<AppQueryResult, ApiError>;
    fn command(
        &self,
        context: &RequestContext,
        envelope: CommandEnvelope,
    ) -> Result<AppCommandResult, ApiError>;
}
