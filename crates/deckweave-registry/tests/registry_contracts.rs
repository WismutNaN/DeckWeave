// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{
    capability::{NodeDescriptor, SoftwareServiceDescriptor},
    ids::{
        CapabilityId, DescriptorRevision, DeviceIdentity, ElementId, NodeId, PackageId,
        PluginSessionId, PortId, ProviderId, RuntimeGroupId, ServiceId, SessionGeneration,
    },
};
use deckweave_registry::{
    CanonicalResourceKey, CapabilityRegistry, EndpointTargetRef, PortPath, RegistryError,
    ResourceLeaseMode, SessionIdentity, StablePortRef,
};
use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

fn fixture(name: &str) -> Result<NodeDescriptor, Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/fixtures/descriptors")
        .join(name);
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn session(generation: u64) -> Result<SessionIdentity, Box<dyn Error>> {
    Ok(SessionIdentity {
        package_id: PackageId::try_new("org.deckweave.fixture.keyboard")?,
        runtime_group_id: RuntimeGroupId::try_new("control")?,
        plugin_session_id: PluginSessionId::try_new(format!("session-{generation}"))?,
        generation: SessionGeneration::try_new(generation)?,
    })
}

fn key_a_reference() -> Result<StablePortRef, Box<dyn Error>> {
    Ok(StablePortRef {
        package_id: PackageId::try_new("org.deckweave.fixture.keyboard")?,
        node_id: NodeId::try_new("local-node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("fixture-hid-provider")?,
            device_identity: DeviceIdentity::try_new("keyboard-observe-fixture")?,
        },
        port: PortPath {
            element_id: ElementId::try_new("key-a")?,
            capability_id: CapabilityId::try_new("key-a-button")?,
            port_id: PortId::try_new("key-a.edge")?,
        },
    })
}

fn same_local_port_reference(target: EndpointTargetRef) -> Result<StablePortRef, Box<dyn Error>> {
    Ok(StablePortRef {
        package_id: PackageId::try_new("org.deckweave.fixture.keyboard")?,
        node_id: NodeId::try_new("local-node")?,
        target,
        port: PortPath {
            element_id: ElementId::try_new("key-a")?,
            capability_id: CapabilityId::try_new("key-a-button")?,
            port_id: PortId::try_new("key-a.edge")?,
        },
    })
}

#[test]
fn delta_is_atomic_and_removed_ports_never_rebind_by_position() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let session = session(1)?;
    registry.start_session(session.clone())?;
    registry.register_snapshot(
        &session,
        DescriptorRevision::try_new(1)?,
        vec![fixture("observe-keyboard.node.json")?],
    )?;
    let reference = key_a_reference()?;
    assert_eq!(registry.resolve(&reference)?.descriptor_revision.get(), 1);

    let error = registry.apply_delta(
        &session,
        DescriptorRevision::try_new(9)?,
        DescriptorRevision::try_new(10)?,
        vec![fixture("analog-keyboard.node.json")?],
        &[],
    );
    assert_eq!(error, Err(RegistryError::ResyncRequired));
    assert!(registry.resolve(&reference).is_ok());

    registry.apply_delta(
        &session,
        DescriptorRevision::try_new(1)?,
        DescriptorRevision::try_new(2)?,
        vec![fixture("analog-keyboard.node.json")?],
        &[],
    )?;
    assert_eq!(
        registry.resolve(&reference),
        Err(RegistryError::PortUnavailable)
    );
    assert_eq!(
        registry.apply_delta(
            &session,
            DescriptorRevision::try_new(2)?,
            DescriptorRevision::try_new(2)?,
            vec![],
            &[],
        ),
        Err(RegistryError::StaleDescriptorRevision)
    );
    assert_eq!(
        registry.register_snapshot(
            &session,
            DescriptorRevision::try_new(1)?,
            vec![fixture("observe-keyboard.node.json")?],
        ),
        Err(RegistryError::StaleDescriptorRevision)
    );
    Ok(())
}

#[test]
fn restart_invalidates_old_session_generation() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let first = session(1)?;
    let second = session(2)?;
    registry.start_session(first.clone())?;
    registry.register_snapshot(
        &first,
        DescriptorRevision::try_new(1)?,
        vec![fixture("observe-keyboard.node.json")?],
    )?;
    registry.start_session(second.clone())?;
    assert_eq!(
        registry.invalidate_session(&first),
        Err(RegistryError::StaleSession)
    );
    assert!(registry.snapshot()?.nodes.is_empty());
    Ok(())
}

#[test]
fn snapshot_revision_cannot_be_reused_for_different_descriptors() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let identity = session(1)?;
    registry.start_session(identity.clone())?;
    let revision = DescriptorRevision::try_new(1)?;
    let node = fixture("observe-keyboard.node.json")?;
    registry.register_snapshot(&identity, revision, vec![node.clone()])?;
    let before = registry.snapshot()?;
    registry.register_snapshot(&identity, revision, vec![node])?;
    assert_eq!(
        registry.snapshot()?,
        before,
        "identical retry must be a no-op"
    );
    assert_eq!(
        registry.register_snapshot(
            &identity,
            revision,
            vec![fixture("analog-keyboard.node.json")?]
        ),
        Err(RegistryError::StaleDescriptorRevision)
    );
    assert_eq!(registry.snapshot()?, before);
    Ok(())
}

#[test]
fn empty_snapshot_still_consumes_its_revision() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let identity = session(1)?;
    registry.start_session(identity.clone())?;
    let revision = DescriptorRevision::try_new(1)?;
    registry.register_snapshot(&identity, revision, vec![])?;
    assert_eq!(
        registry.register_snapshot(
            &identity,
            revision,
            vec![fixture("observe-keyboard.node.json")?]
        ),
        Err(RegistryError::StaleDescriptorRevision)
    );
    Ok(())
}

#[test]
fn delta_requires_an_initial_snapshot() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let identity = session(1)?;
    registry.start_session(identity.clone())?;
    assert_eq!(
        registry.apply_delta(
            &identity,
            DescriptorRevision::try_new(1)?,
            DescriptorRevision::try_new(2)?,
            vec![],
            &[]
        ),
        Err(RegistryError::ResyncRequired)
    );
    Ok(())
}

#[test]
fn canonical_resource_arbiter_detects_provider_alias_conflicts() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let resource = CanonicalResourceKey::try_new("local/usb/5548:1008/interface-1")?;
    let first = registry.acquire_resource(
        PackageId::try_new("org.deckweave.provider.first")?,
        resource.clone(),
        ResourceLeaseMode::Shared,
        "discovery",
    )?;
    let second = registry.acquire_resource(
        PackageId::try_new("org.deckweave.provider.second")?,
        resource.clone(),
        ResourceLeaseMode::Shared,
        "observe",
    )?;
    assert_eq!(
        registry.acquire_resource(
            PackageId::try_new("org.deckweave.provider.probe")?,
            resource,
            ResourceLeaseMode::Exclusive,
            "device-studio-probe",
        ),
        Err(RegistryError::ResourceConflict)
    );
    registry.release_resource(&first.lease_id)?;
    registry.release_resource(&second.lease_id)?;
    Ok(())
}

#[test]
fn identical_devices_and_service_resolve_by_full_stable_path() -> Result<(), Box<dyn Error>> {
    let mut node = fixture("observe-keyboard.node.json")?;
    let first_device = node.devices[0].clone();
    let mut second_device = first_device.clone();
    second_device.identity = DeviceIdentity::try_new("keyboard-observe-fixture-2")?;
    node.devices.push(second_device);
    node.services.push(SoftwareServiceDescriptor {
        service_id: ServiceId::try_new("keyboard-service")?,
        provider_id: ProviderId::try_new("fixture-service-provider")?,
        label: "Keyboard-compatible service".into(),
        elements: first_device.elements.clone(),
        extensions: BTreeMap::new(),
    });

    let first = key_a_reference()?;
    let second = same_local_port_reference(EndpointTargetRef::Device {
        provider_id: ProviderId::try_new("fixture-hid-provider")?,
        device_identity: DeviceIdentity::try_new("keyboard-observe-fixture-2")?,
    })?;
    let service = same_local_port_reference(EndpointTargetRef::Service {
        provider_id: ProviderId::try_new("fixture-service-provider")?,
        service_id: ServiceId::try_new("keyboard-service")?,
    })?;

    let identity = session(1)?;
    let mut registry = CapabilityRegistry::default();
    registry.start_session(identity.clone())?;
    registry.register_snapshot(
        &identity,
        DescriptorRevision::try_new(1)?,
        vec![node.clone()],
    )?;
    assert!(registry.resolve(&first).is_ok());
    assert!(registry.resolve(&second).is_ok());
    assert!(registry.resolve(&service).is_ok());

    node.devices.remove(0);
    node.descriptor_revision = DescriptorRevision::try_new(2)?;
    for device in &mut node.devices {
        for element in &mut device.elements {
            for capability in &mut element.capabilities {
                for port in &mut capability.ports {
                    port.descriptor_revision = DescriptorRevision::try_new(2)?;
                }
            }
        }
    }
    for service_descriptor in &mut node.services {
        for element in &mut service_descriptor.elements {
            for capability in &mut element.capabilities {
                for port in &mut capability.ports {
                    port.descriptor_revision = DescriptorRevision::try_new(2)?;
                }
            }
        }
    }
    registry.register_snapshot(&identity, DescriptorRevision::try_new(2)?, vec![node])?;
    assert_eq!(
        registry.resolve(&first),
        Err(RegistryError::PortUnavailable)
    );
    assert!(registry.resolve(&second).is_ok());
    assert!(registry.resolve(&service).is_ok());
    Ok(())
}

#[test]
fn stable_reference_survives_runtime_group_migration() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let first = session(1)?;
    let reference = key_a_reference()?;
    registry.start_session(first.clone())?;
    registry.register_snapshot(
        &first,
        DescriptorRevision::try_new(1)?,
        vec![fixture("observe-keyboard.node.json")?],
    )?;
    registry.invalidate_session(&first)?;

    let migrated = SessionIdentity {
        package_id: first.package_id,
        runtime_group_id: RuntimeGroupId::try_new("isolated-input")?,
        plugin_session_id: PluginSessionId::try_new("session-migrated")?,
        generation: SessionGeneration::try_new(1)?,
    };
    registry.start_session(migrated.clone())?;
    registry.register_snapshot(
        &migrated,
        DescriptorRevision::try_new(1)?,
        vec![fixture("observe-keyboard.node.json")?],
    )?;
    assert_eq!(
        registry.resolve(&reference)?.session.runtime_group_id,
        migrated.runtime_group_id
    );
    Ok(())
}
