// SPDX-License-Identifier: GPL-3.0-or-later
//! Host lifecycle ownership. API dispatch lives in deckweave-application.

use deckweave_application::{HostSnapshot, HostStatusSource};
use deckweave_domain::LifecycleState;
use deckweave_platform::{HostLease, HostPlatform, PlatformError};
use std::sync::{Arc, Mutex};

mod planning;
pub use deckweave_workspace::ActiveSelection;
pub use planning::*;
mod apply;
pub use apply::*;

pub struct HostRuntime {
    platform: Arc<dyn HostPlatform>,
    _lease: Box<dyn HostLease>,
    state: Mutex<(LifecycleState, Option<String>)>,
}

impl HostRuntime {
    /// Locks are acquired before runtime consumers are constructed.
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

    #[must_use]
    pub fn snapshot(&self) -> HostSnapshot {
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

impl HostStatusSource for HostRuntime {
    fn snapshot(&self) -> HostSnapshot {
        self.snapshot()
    }
}
