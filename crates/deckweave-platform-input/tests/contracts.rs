// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{DeviceIdentity, ProviderId, SessionGeneration};
use deckweave_platform_input::{
    CaptureMode, CapturePolicy, InputContractError, InputEmitterDescriptor, InputGranularity,
    InputObservation, InputOrigin, InputSourceDescriptor, KeyEdge, KeyLocation,
    NormalizedKeyIdentity, PrivilegeAvailability, PrivilegeStatus, validate_capture,
    validate_privilege, validate_source,
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

#[test]
fn observation_keeps_repeat_origin_and_location_explicit() -> Result<(), Box<dyn Error>> {
    let observation = InputObservation {
        key: NormalizedKeyIdentity {
            key: deckweave_domain::ids::NamespacedKind::try_new("org.deckweave.key.keyboard_a")?,
            location: KeyLocation::Standard,
        },
        edge: KeyEdge::Repeated,
        origin: InputOrigin::DeviceOrUnknown,
    };
    let json = serde_json::to_value(&observation)?;
    assert_eq!(json["edge"], "repeated");
    assert_eq!(json["origin"]["kind"], "device_or_unknown");
    assert_eq!(
        serde_json::from_value::<InputObservation>(json)?,
        observation
    );
    Ok(())
}

#[test]
fn synthetic_origin_serializes_stable_provider_and_generation_fields() -> Result<(), Box<dyn Error>>
{
    let origin = InputOrigin::DeckWeaveSynthetic {
        provider_id: ProviderId::try_new("org.deckweave.platform.windows")?,
        runtime_generation: SessionGeneration::try_new(7)?,
    };
    let json = serde_json::to_value(&origin)?;
    assert_eq!(json["kind"], "deck_weave_synthetic");
    assert_eq!(json["providerId"], "org.deckweave.platform.windows");
    assert_eq!(json["runtimeGeneration"], 7);
    assert_eq!(serde_json::from_value::<InputOrigin>(json)?, origin);
    Ok(())
}

#[test]
fn unavailable_privilege_requires_a_machine_readable_reason() -> Result<(), Box<dyn Error>> {
    let capability = deckweave_domain::ids::NamespacedKind::try_new("input.observe")?;
    let missing = PrivilegeStatus {
        capability: capability.clone(),
        availability: PrivilegeAvailability::Unavailable,
        reason: None,
    };
    assert_eq!(
        validate_privilege(&missing),
        Err(InputContractError::MissingPrivilegeReason)
    );
    validate_privilege(&PrivilegeStatus {
        capability,
        availability: PrivilegeAvailability::Degraded,
        reason: Some(deckweave_domain::ids::NamespacedKind::try_new(
            "org.deckweave.input.helper_missing",
        )?),
    })?;
    Ok(())
}
