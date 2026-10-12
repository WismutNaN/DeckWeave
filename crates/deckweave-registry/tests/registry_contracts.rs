// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{
    capability::{NodeDescriptor, SoftwareServiceDescriptor},
    ids::{
        CapabilityId, DescriptorRevision, DeviceIdentity, ElementId, NodeId, OperationId,
        PackageId, PluginSessionId, PortId, ProviderId, RuntimeGroupId, ServiceId,
        SessionGeneration,
    },
};
use deckweave_registry::{
    CanonicalResourceKey, CapabilityRegistry, EndpointTargetRef, PortPath, RegistryError,
    ResourceLeaseMode, ResourceLeaseOwner, ResourceOperationKind, SessionIdentity, StablePortRef,
};
use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

fn fixture(name: &str) -> Result<NodeDescriptor, Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/fixtures/descriptors")
        .join(name);
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn session(generation: u64) -> Result<SessionIdentity, Box<dyn Error>> {
    grouped_session("control", generation)
}

fn grouped_session(group: &str, generation: u64) -> Result<SessionIdentity, Box<dyn Error>> {
    Ok(SessionIdentity {
        package_id: PackageId::try_new("org.deckweave.fixture.keyboard")?,
        runtime_group_id: RuntimeGroupId::try_new(group)?,
        plugin_session_id: PluginSessionId::try_new(format!("session-{group}-{generation}"))?,
        generation: SessionGeneration::try_new(generation)?,
    })
}

fn operation_owner(
    package: &str,
    operation: &str,
    kind: ResourceOperationKind,
    expires_at_host_ticks: u64,
) -> Result<ResourceLeaseOwner, Box<dyn Error>> {
    Ok(ResourceLeaseOwner::Operation {
        package_id: PackageId::try_new(package)?,
        operation_id: OperationId::try_new(operation)?,
        kind,
        expires_at_host_ticks,
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
    registry.register_initial_snapshot(
        session.clone(),
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
    registry.register_initial_snapshot(
        first.clone(),
        DescriptorRevision::try_new(1)?,
        vec![fixture("observe-keyboard.node.json")?],
    )?;
    registry.register_initial_snapshot(second.clone(), DescriptorRevision::try_new(1)?, vec![])?;
    assert_eq!(
        registry.invalidate_session(&first),
        Err(RegistryError::StaleSession)
    );
    assert!(registry.snapshot()?.nodes.is_empty());
    Ok(())
}

#[test]
fn invalid_initial_snapshot_preserves_working_session_and_resources() -> Result<(), Box<dyn Error>>
{
    let mut registry = CapabilityRegistry::default();
    let current = session(1)?;
    let replacement = session(2)?;
    let node = fixture("observe-keyboard.node.json")?;
    let resource = CanonicalResourceKey::try_new("local/usb/active/interface-1")?;
    registry.register_initial_snapshot(
        current.clone(),
        DescriptorRevision::try_new(1)?,
        vec![node.clone()],
    )?;
    registry.acquire_resource(
        ResourceLeaseOwner::RuntimeSession {
            session: current.clone(),
        },
        resource.clone(),
        ResourceLeaseMode::Exclusive,
        "active-runtime",
    )?;
    let before = registry.snapshot()?;

    assert_eq!(
        registry.register_initial_snapshot(
            replacement.clone(),
            DescriptorRevision::try_new(1)?,
            vec![node.clone(), node],
        ),
        Err(RegistryError::DuplicateNode("within snapshot".into()))
    );
    assert_eq!(registry.snapshot()?, before);
    assert_eq!(registry.resolve(&key_a_reference()?)?.session, current);
    assert_eq!(
        registry.acquire_resource(
            ResourceLeaseOwner::RuntimeSession {
                session: replacement,
            },
            resource,
            ResourceLeaseMode::Shared,
            "invalid-replacement",
        ),
        Err(RegistryError::StaleSession)
    );
    Ok(())
}

#[test]
fn snapshot_revision_cannot_be_reused_for_different_descriptors() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let identity = session(1)?;
    let revision = DescriptorRevision::try_new(1)?;
    let node = fixture("observe-keyboard.node.json")?;
    registry.register_initial_snapshot(identity.clone(), revision, vec![node.clone()])?;
    let before = registry.snapshot()?;
    registry.register_initial_snapshot(identity.clone(), revision, vec![node.clone()])?;
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
    let revision = DescriptorRevision::try_new(1)?;
    registry.register_initial_snapshot(identity.clone(), revision, vec![])?;
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
fn delta_rejects_a_session_without_an_initial_snapshot() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let identity = session(1)?;
    assert_eq!(
        registry.apply_delta(
            &identity,
            DescriptorRevision::try_new(1)?,
            DescriptorRevision::try_new(2)?,
            vec![],
            &[]
        ),
        Err(RegistryError::StaleSession)
    );
    Ok(())
}

#[test]
fn canonical_resource_arbiter_detects_provider_alias_conflicts() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let resource = CanonicalResourceKey::try_new("local/usb/5548:1008/interface-1")?;
    let first = registry.acquire_resource(
        operation_owner(
            "org.deckweave.provider.first",
            "discover-first",
            ResourceOperationKind::Probe,
            100,
        )?,
        resource.clone(),
        ResourceLeaseMode::Shared,
        "discovery",
    )?;
    let second = registry.acquire_resource(
        operation_owner(
            "org.deckweave.provider.second",
            "observe-second",
            ResourceOperationKind::Prepare,
            100,
        )?,
        resource.clone(),
        ResourceLeaseMode::Shared,
        "observe",
    )?;
    assert_eq!(
        registry.acquire_resource(
            operation_owner(
                "org.deckweave.provider.probe",
                "exclusive-probe",
                ResourceOperationKind::Probe,
                100,
            )?,
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
fn session_invalidation_releases_only_its_resource_leases() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let group_a = grouped_session("group-a", 1)?;
    let group_b = grouped_session("group-b", 1)?;
    registry.register_initial_snapshot(group_a.clone(), DescriptorRevision::try_new(1)?, vec![])?;
    registry.register_initial_snapshot(group_b.clone(), DescriptorRevision::try_new(1)?, vec![])?;

    let resource_a = CanonicalResourceKey::try_new("local/usb/a/interface-1")?;
    let resource_b = CanonicalResourceKey::try_new("local/usb/b/interface-1")?;
    registry.acquire_resource(
        ResourceLeaseOwner::RuntimeSession {
            session: group_a.clone(),
        },
        resource_a.clone(),
        ResourceLeaseMode::Exclusive,
        "group-a-control",
    )?;
    registry.acquire_resource(
        ResourceLeaseOwner::RuntimeSession {
            session: group_b.clone(),
        },
        resource_b.clone(),
        ResourceLeaseMode::Exclusive,
        "group-b-control",
    )?;

    registry.invalidate_session(&group_a)?;
    assert_eq!(
        registry.acquire_resource(
            ResourceLeaseOwner::RuntimeSession {
                session: group_a.clone(),
            },
            resource_a.clone(),
            ResourceLeaseMode::Shared,
            "stale-runtime",
        ),
        Err(RegistryError::StaleSession)
    );
    let probe = operation_owner(
        "org.deckweave.provider.probe",
        "probe-after-a-crash",
        ResourceOperationKind::Probe,
        100,
    )?;
    assert!(
        registry
            .acquire_resource(
                probe.clone(),
                resource_a,
                ResourceLeaseMode::Exclusive,
                "probe-a",
            )
            .is_ok()
    );
    assert_eq!(
        registry.acquire_resource(probe, resource_b, ResourceLeaseMode::Exclusive, "probe-b",),
        Err(RegistryError::ResourceConflict)
    );
    let leases = registry.snapshot()?.resource_leases;
    assert_eq!(leases.len(), 2);
    assert!(leases.iter().any(|lease| {
        lease.owner
            == ResourceLeaseOwner::RuntimeSession {
                session: group_b.clone(),
            }
    }));
    Ok(())
}

#[test]
fn stale_session_cleanup_cannot_release_replacement_resources() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let old = session(1)?;
    let current = session(2)?;
    let resource = CanonicalResourceKey::try_new("local/usb/replaced/interface-1")?;
    registry.register_initial_snapshot(old.clone(), DescriptorRevision::try_new(1)?, vec![])?;
    registry.acquire_resource(
        ResourceLeaseOwner::RuntimeSession {
            session: old.clone(),
        },
        resource.clone(),
        ResourceLeaseMode::Exclusive,
        "old-runtime",
    )?;

    registry.register_initial_snapshot(current.clone(), DescriptorRevision::try_new(1)?, vec![])?;
    registry.acquire_resource(
        ResourceLeaseOwner::RuntimeSession {
            session: current.clone(),
        },
        resource.clone(),
        ResourceLeaseMode::Exclusive,
        "current-runtime",
    )?;
    assert_eq!(
        registry.invalidate_session(&old),
        Err(RegistryError::StaleSession)
    );
    assert_eq!(
        registry.acquire_resource(
            operation_owner(
                "org.deckweave.provider.probe",
                "probe-current-resource",
                ResourceOperationKind::Probe,
                100,
            )?,
            resource,
            ResourceLeaseMode::Exclusive,
            "must-conflict",
        ),
        Err(RegistryError::ResourceConflict)
    );
    Ok(())
}

#[test]
fn operation_resource_leases_expire_and_cancel_by_exact_owner() -> Result<(), Box<dyn Error>> {
    let mut registry = CapabilityRegistry::default();
    let prepare = operation_owner(
        "org.deckweave.provider.prepare",
        "prepare-1",
        ResourceOperationKind::Prepare,
        50,
    )?;
    let probe = operation_owner(
        "org.deckweave.provider.probe",
        "probe-1",
        ResourceOperationKind::Probe,
        100,
    )?;
    registry.acquire_resource(
        prepare.clone(),
        CanonicalResourceKey::try_new("local/usb/prepare/interface-1")?,
        ResourceLeaseMode::Exclusive,
        "prepare",
    )?;
    registry.acquire_resource(
        probe.clone(),
        CanonicalResourceKey::try_new("local/usb/probe/interface-1")?,
        ResourceLeaseMode::Exclusive,
        "probe",
    )?;

    assert_eq!(registry.expire_resource_operations(49)?, 0);
    assert_eq!(registry.expire_resource_operations(50)?, 1);
    assert_eq!(registry.snapshot()?.resource_leases.len(), 1);
    assert_eq!(registry.release_resources_for_owner(&probe)?, 1);
    assert_eq!(registry.release_resources_for_owner(&probe)?, 0);
    assert!(registry.snapshot()?.resource_leases.is_empty());
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
    registry.register_initial_snapshot(
        identity.clone(),
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
    registry.register_initial_snapshot(
        first.clone(),
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
    registry.register_initial_snapshot(
        migrated.clone(),
        DescriptorRevision::try_new(1)?,
        vec![fixture("observe-keyboard.node.json")?],
    )?;
    assert_eq!(
        registry.resolve(&reference)?.session.runtime_group_id,
        migrated.runtime_group_id
    );
    Ok(())
}
