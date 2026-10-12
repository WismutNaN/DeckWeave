// SPDX-License-Identifier: GPL-3.0-or-later
//! Application-domain vocabulary. Wire DTOs remain in `deckweave-protocol`.

pub mod automation;
pub mod capability;
pub mod endpoint;
pub mod ids;
pub mod lighting;
pub mod media;
pub mod remote;
pub mod schema;
pub mod security;
pub mod visual;

use serde::{Deserialize, Serialize};

/// Состояние процесса host независимо от видимости окна.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Running,
    Stopping,
}

/// Политика размещения данных и автозапуска.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    Installed,
    Portable,
}
