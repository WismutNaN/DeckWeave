// SPDX-License-Identifier: GPL-3.0-or-later
use super::model::{
    AudioAvailability, AudioCommand, AudioCommandError, AudioFailure, AudioReading, AudioState,
};

pub(super) trait AudioBackend {
    fn read(&mut self) -> Result<AudioReading, AudioFailure>;
    fn set_volume(&mut self, endpoint: &str, scalar: f64) -> Result<(), AudioCommandError>;
    fn set_mute(&mut self, endpoint: &str, muted: bool) -> Result<(), AudioCommandError>;
    fn maintenance_due(&mut self) -> bool;
}

pub(super) struct AudioEngine<B> {
    backend: B,
    last_state: Option<AudioState>,
    next_revision: u64,
}

impl<B: AudioBackend> AudioEngine<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            last_state: None,
            next_revision: 1,
        }
    }

    pub fn apply(&mut self, command: AudioCommand) -> Result<AudioState, AudioCommandError> {
        // Select once. Relative changes and confirmation must refer to this same endpoint.
        match command {
            AudioCommand::SetVolume(value) => validate_scalar(value)?,
            AudioCommand::ChangeVolume(value) => validate_delta(value)?,
            _ => {}
        }
        let current = self
            .backend
            .read()
            .map_err(AudioCommandError::before_effect)?;
        match command {
            AudioCommand::SetVolume(scalar) => {
                self.backend.set_volume(&current.endpoint_ref, scalar)?
            }
            AudioCommand::ChangeVolume(delta) => self.backend.set_volume(
                &current.endpoint_ref,
                (current.scalar + delta).clamp(0.0, 1.0),
            )?,
            AudioCommand::SetMute(muted) => self.backend.set_mute(&current.endpoint_ref, muted)?,
            AudioCommand::ToggleMute => self
                .backend
                .set_mute(&current.endpoint_ref, !current.muted)?,
        }
        self.confirm(&current.endpoint_ref)
            .map_err(AudioCommandError::after_effect)
    }

    pub fn refresh(&mut self) -> AudioState {
        match self.backend.read() {
            Ok(reading) => self.record(
                reading.endpoint_ref,
                reading.scalar,
                reading.muted,
                AudioAvailability::Available,
            ),
            Err(failure) => self.record_failure(failure),
        }
    }

    pub fn maintenance_due(&mut self) -> bool {
        self.backend.maintenance_due()
    }

    fn confirm(&mut self, endpoint: &str) -> Result<AudioState, AudioFailure> {
        let reading = self.backend.read()?;
        if reading.endpoint_ref != endpoint {
            return Err(AudioFailure::TransientRebind);
        }
        Ok(self.record(
            reading.endpoint_ref,
            reading.scalar,
            reading.muted,
            AudioAvailability::Available,
        ))
    }

    fn record_failure(&mut self, failure: AudioFailure) -> AudioState {
        let availability = if failure == AudioFailure::TransientRebind {
            AudioAvailability::Rebind
        } else {
            AudioAvailability::Unavailable
        };
        self.record("default-render-multimedia".into(), 0.0, false, availability)
    }

    fn record(
        &mut self,
        endpoint_ref: String,
        scalar: f64,
        muted: bool,
        availability: AudioAvailability,
    ) -> AudioState {
        let unchanged = self.last_state.as_ref().is_some_and(|state| {
            state.endpoint_ref == endpoint_ref
                && state.scalar == scalar
                && state.muted == muted
                && state.availability == availability
        });
        if unchanged {
            return self.last_state.clone().unwrap_or(AudioState {
                endpoint_ref,
                scalar,
                muted,
                availability,
                revision: 1,
            });
        }
        let state = AudioState {
            endpoint_ref,
            scalar,
            muted,
            availability,
            revision: self.next_revision,
        };
        self.next_revision = self.next_revision.saturating_add(1);
        self.last_state = Some(state.clone());
        state
    }
}

fn validate_scalar(value: f64) -> Result<(), AudioCommandError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AudioCommandError::before_effect(AudioFailure::InvalidInput));
    }
    Ok(())
}

fn validate_delta(value: f64) -> Result<(), AudioCommandError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(AudioCommandError::before_effect(AudioFailure::InvalidInput));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
