// SPDX-License-Identifier: GPL-3.0-or-later
//! Долгоживущий host. В фазе 1 нет Store, API listener или hardware.

use deckweave_application::{
    ApplicationService, HostLease, HostPlatform, HostSnapshot, PlatformError,
};
use deckweave_domain::LifecycleState;
use std::sync::{Arc, Mutex};

pub struct HostRuntime {
    platform: Arc<dyn HostPlatform>,
    _lease: Box<dyn HostLease>,
    state: Mutex<(LifecycleState, Option<String>)>,
}

impl HostRuntime {
    /// Блокировки приобретаются до появления runtime consumers.
    pub fn start(platform: Arc<dyn HostPlatform>) -> Result<Self, PlatformError> {
        let lease = platform.acquire()?;
        Ok(Self {
            platform,
            _lease: lease,
            state: Mutex::new((LifecycleState::Running, None)),
        })
    }

    pub fn stop(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0 = LifecycleState::Stopping;
    }

    pub fn set_notice(&self, notice: Option<String>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1 = notice;
    }

    pub fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        self.platform.autostart_enabled()
    }

    pub fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError> {
        self.platform.set_autostart(enabled)
    }
}

impl ApplicationService for HostRuntime {
    fn snapshot(&self) -> HostSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        HostSnapshot {
            lifecycle: state.0,
            notice: state.1.clone(),
            mode: self.platform.mode(),
            data_root: self.platform.data_root().to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}
