// SPDX-License-Identifier: GPL-3.0-or-later
use super::model::{
    AudioAvailability, AudioCommand, AudioCommandError, AudioFailure, AudioReading, AudioState,
};

pub(super) trait AudioBackend {
    fn read(&mut self) -> Result<AudioReading, AudioFailure>;
    fn set_volume(&mut self, scalar: f64) -> Result<(), AudioFailure>;
    fn set_mute(&mut self, muted: bool) -> Result<(), AudioFailure>;
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
        match command {
            AudioCommand::SetVolume(scalar) => {
                validate_scalar(scalar)?;
                self.backend
                    .set_volume(scalar)
                    .map_err(AudioCommandError::before_effect)?;
            }
            AudioCommand::ChangeVolume(delta) => {
                validate_delta(delta)?;
                let current = self
                    .backend
                    .read()
                    .map_err(AudioCommandError::before_effect)?;
                self.backend
                    .set_volume((current.scalar + delta).clamp(0.0, 1.0))
                    .map_err(AudioCommandError::before_effect)?;
            }
            AudioCommand::SetMute(muted) => {
                self.backend
                    .set_mute(muted)
                    .map_err(AudioCommandError::before_effect)?;
            }
            AudioCommand::ToggleMute => {
                let current = self
                    .backend
                    .read()
                    .map_err(AudioCommandError::before_effect)?;
                self.backend
                    .set_mute(!current.muted)
                    .map_err(AudioCommandError::before_effect)?;
            }
        }
        self.confirm().map_err(AudioCommandError::after_effect)
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

    fn confirm(&mut self) -> Result<AudioState, AudioFailure> {
        let reading = self.backend.read()?;
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
mod tests {
    use super::*;
    use crate::audio::EffectKnowledge;

    struct FakeBackend {
        reading: AudioReading,
        fail_next_read: bool,
        fail_after_set: bool,
        volume_sets: Vec<f64>,
        mute_sets: Vec<bool>,
    }

    impl FakeBackend {
        fn available() -> Self {
            Self {
                reading: AudioReading {
                    endpoint_ref: "endpoint-a".into(),
                    scalar: 0.8,
                    muted: false,
                },
                fail_next_read: false,
                fail_after_set: false,
                volume_sets: vec![],
                mute_sets: vec![],
            }
        }
    }

    impl AudioBackend for FakeBackend {
        fn read(&mut self) -> Result<AudioReading, AudioFailure> {
            if self.fail_next_read {
                self.fail_next_read = false;
                return Err(AudioFailure::TransientRebind);
            }
            Ok(self.reading.clone())
        }

        fn set_volume(&mut self, scalar: f64) -> Result<(), AudioFailure> {
            self.volume_sets.push(scalar);
            self.reading.scalar = scalar;
            self.fail_next_read = self.fail_after_set;
            Ok(())
        }

        fn set_mute(&mut self, muted: bool) -> Result<(), AudioFailure> {
            self.mute_sets.push(muted);
            self.reading.muted = muted;
            self.fail_next_read = self.fail_after_set;
            Ok(())
        }

        fn maintenance_due(&mut self) -> bool {
            false
        }
    }

    #[test]
    fn relative_volume_clamps_before_the_backend_and_returns_confirmed_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = AudioEngine::new(FakeBackend::available());
        let state = engine.apply(AudioCommand::ChangeVolume(0.5))?;
        assert_eq!(engine.backend.volume_sets, vec![1.0]);
        assert_eq!(state.scalar, 1.0);
        assert_eq!(state.availability, AudioAvailability::Available);
        Ok(())
    }

    #[test]
    fn invalid_scalar_is_rejected_without_calling_the_backend()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut engine = AudioEngine::new(FakeBackend::available());
        let error = engine
            .apply(AudioCommand::SetVolume(f64::NAN))
            .err()
            .ok_or("NaN was accepted")?;
        assert_eq!(error.failure, AudioFailure::InvalidInput);
        assert_eq!(error.effect, EffectKnowledge::NotApplied);
        assert!(engine.backend.volume_sets.is_empty());
        Ok(())
    }

    #[test]
    fn failed_confirmation_after_a_successful_set_is_not_reported_as_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = FakeBackend::available();
        backend.fail_after_set = true;
        let mut engine = AudioEngine::new(backend);
        let error = engine
            .apply(AudioCommand::SetMute(true))
            .err()
            .ok_or("failed confirmation was reported as success")?;
        assert_eq!(error.failure, AudioFailure::TransientRebind);
        assert_eq!(error.effect, EffectKnowledge::PossiblyApplied);
        assert_eq!(engine.backend.mute_sets, vec![true]);
        Ok(())
    }

    #[test]
    fn unchanged_observation_keeps_the_same_revision() {
        let mut engine = AudioEngine::new(FakeBackend::available());
        let first = engine.refresh();
        let second = engine.refresh();
        assert_eq!(first.revision, second.revision);
    }
}
