// SPDX-License-Identifier: GPL-3.0-or-later
mod model;
mod service;
mod worker;

#[cfg(not(windows))]
mod unsupported_backend;
#[cfg(windows)]
mod windows_backend;

pub use model::{
    AudioAvailability, AudioCommand, AudioCommandError, AudioFailure, AudioState, EffectKnowledge,
};
pub use worker::{AudioClient, AudioStateFeed, AudioWorker};
