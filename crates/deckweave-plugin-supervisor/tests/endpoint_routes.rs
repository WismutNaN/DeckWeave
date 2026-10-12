// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        BindingId, DescriptorRevision, PackageId, PluginSessionId, RuntimeGroupId,
        SessionGeneration,
    },
};
use deckweave_plugin_supervisor::routing::{
    EndpointRoutingError, normalize_signal_source, resolve_endpoint_binding,
};
use deckweave_protocol::v1::{EndpointPath, Signal, endpoint_path};
use deckweave_registry::{CapabilityRegistry, SessionIdentity};
use deckweave_workspace::EndpointBinding;
use std::{error::Error, fs, path::PathBuf};

fn fixture() -> Result<NodeDescriptor, Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/fixtures/descriptors/observe-keyboard.node.json");
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn session() -> Result<SessionIdentity, Box<dyn Error>> {
    Ok(SessionIdentity {
        package_id: PackageId::try_new("org.deckweave.fixture.keyboard")?,
        runtime_group_id: RuntimeGroupId::try_new("input")?,
        plugin_session_id: PluginSessionId::try_new("plugin-session-1")?,
        generation: SessionGeneration::try_new(3)?,
    })
}

fn key_a_path() -> EndpointPath {
    EndpointPath {
        node_id: "local-node".into(),
        provider_id: "fixture-hid-provider".into(),
        target: Some(endpoint_path::Target::DeviceIdentity(
            "keyboard-observe-fixture".into(),
        )),
        element_id: "key-a".into(),
        capability_id: "key-a-button".into(),
        port_id: "key-a.edge".into(),
    }
}

#[test]
fn wire_signal_binding_and_registry_share_one_endpoint_identity() -> Result<(), Box<dyn Error>> {
    let session = session()?;
    let revision = DescriptorRevision::try_new(1)?;
    let mut registry = CapabilityRegistry::default();
    registry.register_initial_snapshot(session.clone(), revision, vec![fixture()?])?;

    let signal = Signal {
        source: Some(key_a_path()),
        ..Default::default()
    };
    let normalized = normalize_signal_source(&session, revision, &signal)?;
    assert_eq!(registry.resolve(&normalized.stable)?, normalized);

    let binding = EndpointBinding {
        binding_id: BindingId::try_new("key-a-loopback")?,
        source: normalized.stable.clone(),
        target: normalized.stable.clone(),
        enabled: true,
        extensions: Default::default(),
    };
    let route = resolve_endpoint_binding(&registry, &binding)?;
    assert_eq!(route.source, normalized);
    assert_eq!(route.target, normalized);
    Ok(())
}

#[test]
#[allow(deprecated)]
fn legacy_local_port_id_is_never_guessed() -> Result<(), Box<dyn Error>> {
    let signal = Signal {
        source_port_id: "key-a.edge".into(),
        ..Default::default()
    };
    assert_eq!(
        normalize_signal_source(&session()?, DescriptorRevision::try_new(1)?, &signal),
        Err(EndpointRoutingError::MissingSource)
    );
    Ok(())
}
