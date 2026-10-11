// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        DescriptorRevision, NodeId, PackageId, PluginSessionId, PortId, RuntimeGroupId,
        SessionGeneration,
    },
};
use deckweave_registry::{
    CanonicalResourceKey, CapabilityRegistry, RegistryError, ResourceLeaseMode, SessionIdentity,
    StablePortRef,
};
use std::{error::Error, fs, path::PathBuf};

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
        runtime_group_id: RuntimeGroupId::try_new("control")?,
        node_id: NodeId::try_new("local-node")?,
        port_id: PortId::try_new("key-a.edge")?,
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
