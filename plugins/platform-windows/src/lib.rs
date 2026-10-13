// SPDX-License-Identifier: GPL-3.0-or-later
//! First-party Windows package shell. OS adapters remain controlled until
//! their corresponding Phase 7 acceptance slice is complete.

pub mod audio;
mod control_audio;
mod control_window;
mod descriptor;
mod error;
mod heartbeat;
mod input;
mod invocation;
mod media;
mod process_broker;
mod runtime;
mod runtime_group;
mod state_gate;
mod window;
mod wire;

pub use error::PlatformFailure;
pub use runtime::run;
pub use runtime_group::RuntimeGroup;
