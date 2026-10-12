// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only projections of current runtime state.

use super::ProductionRuntimeCoordinator;
use crate::host::health::RuntimeHealthSnapshot;
use deckweave_host::LiveExecutionOwner;
use std::sync::Arc;

impl ProductionRuntimeCoordinator {
    #[must_use]
    pub fn runtime_health(&self) -> RuntimeHealthSnapshot {
        self.health.snapshot()
    }

    #[must_use]
    pub fn live_execution(&self) -> Arc<LiveExecutionOwner> {
        Arc::clone(&self.execution)
    }
}
