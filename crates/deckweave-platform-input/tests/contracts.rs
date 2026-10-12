// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{DeviceIdentity, ProviderId};
use deckweave_platform_input::{
    CaptureMode, CapturePolicy, InputContractError, InputEmitterDescriptor, InputGranularity,
    InputSourceDescriptor, validate_capture, validate_source,
};
use std::error::Error;

fn aggregate_keyboard() -> Result<InputSourceDescriptor, Box<dyn Error>> {
    Ok(InputSourceDescriptor {
        provider_id: ProviderId::try_new("builtin-platform-input")?,
        label: "All keyboards".into(),
        granularity: InputGranularity::SystemAggregate,
        identity: None,
        evidence: vec![],
        can_observe: true,
        can_intercept: false,
    })
}

#[test]
fn bundled_aggregate_keyboard_is_observe_only_and_passes_through() -> Result<(), Box<dyn Error>> {
    let source = aggregate_keyboard()?;
    validate_capture(&source, CapturePolicy::observe())?;
    assert_eq!(
        validate_capture(
            &source,
            CapturePolicy {
                mode: CaptureMode::Intercept,
                pass_through: false,
                fail_open: true
            }
        ),
        Err(InputContractError::UnsupportedMode)
    );
    Ok(())
}

#[test]
fn physical_identity_needs_evidence_and_emit_is_a_separate_contract() -> Result<(), Box<dyn Error>>
{
    let physical = InputSourceDescriptor {
        provider_id: ProviderId::try_new("fixture-input")?,
        label: "Keyboard".into(),
        granularity: InputGranularity::PhysicalDevice,
        identity: Some(DeviceIdentity::try_new("keyboard-1")?),
        evidence: vec![],
        can_observe: true,
        can_intercept: true,
    };
    assert_eq!(
        validate_source(&physical),
        Err(InputContractError::MissingPhysicalEvidence)
    );
    let emitter = InputEmitterDescriptor {
        provider_id: ProviderId::try_new("fixture-output")?,
        label: "Virtual keyboard".into(),
        virtual_identity: DeviceIdentity::try_new("virtual-keyboard-1")?,
    };
    assert_ne!(physical.provider_id, emitter.provider_id);
    Ok(())
}
