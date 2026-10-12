// SPDX-License-Identifier: GPL-3.0-or-later
//! Narrow ports implemented by operating-system infrastructure adapters.

use deckweave_domain::DeploymentMode;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
};
use thiserror::Error;

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

/// Durable publication port for JSON source-of-truth adapters.
pub trait AtomicFileStore: Send + Sync {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), StorageError>;
}
