// SPDX-License-Identifier: GPL-3.0-or-later
use super::{NativeWindow, WindowBackend, WindowFailure};
use std::{
    sync::mpsc::{self, Receiver, SyncSender},
    time::Duration,
};

pub(super) struct NativeWindowBackend;

impl NativeWindowBackend {
    pub(super) fn new() -> Self {
        Self
    }
}

impl WindowBackend for NativeWindowBackend {
    fn list_windows(&mut self) -> Result<Vec<NativeWindow>, WindowFailure> {
        Err(WindowFailure::UnsupportedOperatingSystem)
    }

    fn foreground_token(&mut self) -> Result<Option<u64>, WindowFailure> {
        Err(WindowFailure::UnsupportedOperatingSystem)
    }

    fn activate(&mut self, _token: u64, _timeout: Duration) -> Result<(), WindowFailure> {
        Err(WindowFailure::UnsupportedOperatingSystem)
    }
}

pub(super) struct ForegroundHook {
    _keep_open: SyncSender<()>,
}

impl ForegroundHook {
    pub(super) fn start() -> (Self, Receiver<()>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        (Self { _keep_open: sender }, receiver)
    }
}
