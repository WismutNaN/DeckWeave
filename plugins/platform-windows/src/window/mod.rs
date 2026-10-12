// SPDX-License-Identifier: GPL-3.0-or-later
mod model;
mod service;
mod worker;

#[cfg(not(windows))]
mod unsupported_backend;
#[cfg(windows)]
mod windows_backend;

pub(crate) use model::*;
pub(crate) use service::{WindowBackend, WindowService};
pub(crate) use worker::{WindowClient, WindowWorker};
