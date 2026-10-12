// SPDX-License-Identifier: GPL-3.0-or-later
//! First-party Windows package shell. OS adapters remain controlled until
//! their corresponding Phase 7 acceptance slice is complete.

pub mod audio;
mod control_audio;
mod control_window;
mod descriptor;
mod error;
mod runtime;
mod window;
mod wire;

pub use error::PlatformFailure;
pub use runtime::{RuntimeGroup, run};
