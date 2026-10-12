// SPDX-License-Identifier: GPL-3.0-or-later
//! Narrow native adapters for host bootstrap services.

mod launch;
mod native;
mod process;
mod storage;
#[cfg(windows)]
mod windows_autostart;
#[cfg(windows)]
mod windows_process_guard;
#[cfg(windows)]
mod windows_storage;

pub use launch::{LaunchOptions, installed_root};
pub use native::{FileLease, NativePlatform};
pub use process::NativeChildProcessPolicy;
pub use storage::NativeAtomicFileStore;
