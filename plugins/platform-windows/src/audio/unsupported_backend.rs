// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    model::{AudioCommandError, AudioFailure, AudioReading},
    service::AudioBackend,
};

pub(super) struct NativeAudioBackend;

impl NativeAudioBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl AudioBackend for NativeAudioBackend {
    fn read(&mut self) -> Result<AudioReading, AudioFailure> {
        Err(AudioFailure::UnsupportedOperatingSystem)
    }

    fn set_volume(&mut self, _endpoint: &str, _scalar: f64) -> Result<(), AudioCommandError> {
        Err(AudioCommandError::before_effect(
            AudioFailure::UnsupportedOperatingSystem,
        ))
    }

    fn set_mute(&mut self, _endpoint: &str, _muted: bool) -> Result<(), AudioCommandError> {
        Err(AudioCommandError::before_effect(
            AudioFailure::UnsupportedOperatingSystem,
        ))
    }

    fn maintenance_due(&mut self) -> bool {
        false
    }
}
