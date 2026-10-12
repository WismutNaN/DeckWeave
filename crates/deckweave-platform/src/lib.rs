// SPDX-License-Identifier: GPL-3.0-or-later
//! Narrow ports implemented by operating-system infrastructure adapters.

use deckweave_domain::DeploymentMode;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
};
use thiserror::Error;
mod preferences;
pub use preferences::{LocalePreferenceStore, PreferenceError};

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

/// Resources are released by Drop, including abnormal application shutdown.
pub trait HostLease: Send + Sync {}

/// Bootstrap-only services. User-visible OS actions belong to plugins.
pub trait HostPlatform: Send + Sync {
    fn data_root(&self) -> &Path;
    fn mode(&self) -> DeploymentMode;
    fn acquire(&self) -> Result<Box<dyn HostLease>, PlatformError>;
    fn autostart_enabled(&self) -> Result<bool, PlatformError>;
    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError>;
}

pub trait ChildProcessGuard: Send {
    fn terminate_tree(&mut self) -> Result<(), PlatformError>;
}

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

/// What is known after an atomic publication attempt fails.
///
/// A successful [`AtomicFileStore::write_atomic`] always means that the target
/// was observed with the requested bytes. An error must retain whether retrying
/// is safe without first reconciling durable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationOutcome {
    NotPublished,
    Unknown,
}

#[derive(Debug, Error)]
pub enum PublicationError {
    #[error("durable publication did not occur: {0}")]
    NotPublished(#[source] StorageError),
    #[error(
        "durable publication outcome is unknown for {target}; staged path is retained if present at {staged_path}: publish failed: {publish_error}; reconciliation failed: {reconciliation_error}"
    )]
    Unknown {
        target: PathBuf,
        staged_path: PathBuf,
        #[source]
        publish_error: StorageError,
        reconciliation_error: String,
    },
}

impl PublicationError {
    pub fn not_published(error: impl Into<StorageError>) -> Self {
        Self::NotPublished(error.into())
    }

    pub fn unknown(
        target: PathBuf,
        staged_path: PathBuf,
        publish_error: StorageError,
        reconciliation_error: impl Into<String>,
    ) -> Self {
        Self::Unknown {
            target,
            staged_path,
            publish_error,
            reconciliation_error: reconciliation_error.into(),
        }
    }

    pub fn outcome(&self) -> PublicationOutcome {
        match self {
            Self::NotPublished(_) => PublicationOutcome::NotPublished,
            Self::Unknown { .. } => PublicationOutcome::Unknown,
        }
    }

    pub fn staged_path(&self) -> Option<&Path> {
        match self {
            Self::NotPublished(_) => None,
            Self::Unknown { staged_path, .. } => Some(staged_path),
        }
    }
}

/// Durable publication port for JSON source-of-truth adapters.
pub trait AtomicFileStore: Send + Sync {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), PublicationError>;
}
