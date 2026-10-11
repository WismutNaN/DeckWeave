// SPDX-License-Identifier: GPL-3.0-or-later
//! Порты application layer без зависимостей от Tauri и реализаций ОС.

use std::path::PathBuf;

use deckweave_domain::{DeploymentMode, LifecycleState};
use serde::Serialize;
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSnapshot {
    pub lifecycle: LifecycleState,
    pub mode: DeploymentMode,
    pub data_root: PathBuf,
    pub version: String,
    pub notice: Option<String>,
}

/// Начальный read-only контракт. Полный command/query API — фаза 4.
pub trait ApplicationService: Send + Sync {
    fn snapshot(&self) -> HostSnapshot;
}
