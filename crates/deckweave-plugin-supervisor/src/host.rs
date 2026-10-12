// SPDX-License-Identifier: GPL-3.0-or-later
//! Production bridge from Host apply ports to isolated plugin processes.

mod config;
mod coordinator;
mod dispatch;
mod health;
mod instance;
mod ports;

pub use config::{HostRuntimeConfig, HostRuntimeError};
pub use coordinator::ProductionRuntimeCoordinator;
pub use health::{RuntimeHealthEntry, RuntimeHealthSnapshot, RuntimeHealthStatus};
