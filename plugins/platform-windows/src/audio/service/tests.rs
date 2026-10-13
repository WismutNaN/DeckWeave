// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::audio::EffectKnowledge;

struct FakeBackend {
    reading: AudioReading,
    fail_next_read: bool,
    fail_after_set: bool,
    switch_before_set: bool,
    switch_after_set: bool,
    uncertain_set: bool,
    volume_sets: Vec<f64>,
    mute_sets: Vec<bool>,
}

impl FakeBackend {
    fn check_target(&mut self, endpoint: &str) -> Result<(), AudioCommandError> {
        if self.switch_before_set {
            self.reading.endpoint_ref = "endpoint-b".into();
        }
        if self.reading.endpoint_ref != endpoint {
            return Err(AudioCommandError::before_effect(
                AudioFailure::TransientRebind,
            ));
        }
        Ok(())
    }

    fn available() -> Self {
        Self {
            reading: AudioReading {
                endpoint_ref: "endpoint-a".into(),
                scalar: 0.8,
                muted: false,
            },
            fail_next_read: false,
            fail_after_set: false,
            switch_before_set: false,
            switch_after_set: false,
            uncertain_set: false,
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

    fn set_volume(&mut self, endpoint: &str, scalar: f64) -> Result<(), AudioCommandError> {
        self.check_target(endpoint)?;
        self.volume_sets.push(scalar);
        self.reading.scalar = scalar;
        self.fail_next_read = self.fail_after_set;
        if self.switch_after_set {
            self.reading.endpoint_ref = "endpoint-b".into();
        }
        if self.uncertain_set {
            return Err(AudioCommandError::after_effect(
                AudioFailure::OsApiUnavailable,
            ));
        }
        Ok(())
    }

    fn set_mute(&mut self, endpoint: &str, muted: bool) -> Result<(), AudioCommandError> {
        self.check_target(endpoint)?;
        self.mute_sets.push(muted);
        self.reading.muted = muted;
        self.fail_next_read = self.fail_after_set;
        if self.switch_after_set {
            self.reading.endpoint_ref = "endpoint-b".into();
        }
        if self.uncertain_set {
            return Err(AudioCommandError::after_effect(
                AudioFailure::OsApiUnavailable,
            ));
        }
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
fn invalid_scalar_is_rejected_without_calling_the_backend() -> Result<(), Box<dyn std::error::Error>>
{
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

#[test]
fn default_switch_never_applies_a_relative_value_to_another_endpoint()
-> Result<(), Box<dyn std::error::Error>> {
    for command in [
        AudioCommand::SetVolume(0.4),
        AudioCommand::ChangeVolume(0.1),
        AudioCommand::SetMute(true),
        AudioCommand::ToggleMute,
    ] {
        let mut backend = FakeBackend::available();
        backend.switch_before_set = true;
        let mut engine = AudioEngine::new(backend);
        let error = engine
            .apply(command)
            .err()
            .ok_or("switched endpoint accepted")?;
        assert_eq!(error.effect, EffectKnowledge::NotApplied);
        assert!(engine.backend.volume_sets.is_empty());
        assert!(engine.backend.mute_sets.is_empty());
    }
    Ok(())
}

#[test]
fn default_switch_after_set_is_unknown_and_refresh_never_replays_the_effect()
-> Result<(), Box<dyn std::error::Error>> {
    let mut backend = FakeBackend::available();
    backend.switch_after_set = true;
    let mut engine = AudioEngine::new(backend);
    let error = engine
        .apply(AudioCommand::ChangeVolume(0.1))
        .err()
        .ok_or("wrong endpoint confirmed")?;
    assert_eq!(error.effect, EffectKnowledge::PossiblyApplied);
    for _ in 0..5 {
        assert_eq!(engine.refresh().endpoint_ref, "endpoint-b");
    }
    assert_eq!(engine.backend.volume_sets.len(), 1);
    Ok(())
}

#[test]
fn mutation_transport_failure_preserves_uncertainty() -> Result<(), Box<dyn std::error::Error>> {
    let mut backend = FakeBackend::available();
    backend.uncertain_set = true;
    let mut engine = AudioEngine::new(backend);
    assert_eq!(
        engine
            .apply(AudioCommand::ToggleMute)
            .err()
            .ok_or("expected failure")?
            .effect,
        EffectKnowledge::PossiblyApplied
    );
    engine.refresh();
    assert_eq!(engine.backend.mute_sets, vec![true]);
    Ok(())
}
