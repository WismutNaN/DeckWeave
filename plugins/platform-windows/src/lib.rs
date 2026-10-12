// SPDX-License-Identifier: GPL-3.0-or-later
//! First-party Windows package shell. OS adapters remain controlled until
//! their corresponding Phase 7 acceptance slice is complete.

mod error;
mod runtime;

pub use error::PlatformFailure;
pub use runtime::{RuntimeGroup, run};
