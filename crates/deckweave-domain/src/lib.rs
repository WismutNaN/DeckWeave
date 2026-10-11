// SPDX-License-Identifier: GPL-3.0-or-later
//! Минимальное состояние host; модель устройств появится в фазе 2.

use serde::Serialize;

/// Состояние процесса host независимо от видимости окна.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Running,
    Stopping,
}

/// Политика размещения данных и автозапуска.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    Installed,
    Portable,
}
