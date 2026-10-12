// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
    ids::{
        BindingId, CapabilityId, DeviceIdentity, ElementId, NodeId, PackageId, PortId, ProviderId,
    },
};
use deckweave_workspace::EndpointBinding;
use std::error::Error;

fn endpoint(device: &str) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new("org.deckweave.fixture.keyboard")?,
        node_id: NodeId::try_new("local-node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("fixture-hid-provider")?,
            device_identity: DeviceIdentity::try_new(device)?,
        },
        port: PortPath {
            element_id: ElementId::try_new("key-a")?,
            capability_id: CapabilityId::try_new("key-a-button")?,
            port_id: PortId::try_new("key-a.edge")?,
        },
    })
}

#[test]
fn persisted_binding_keeps_full_stable_identity_without_runtime_fences()
-> Result<(), Box<dyn Error>> {
    let binding = EndpointBinding {
        binding_id: BindingId::try_new("same-local-port")?,
        source: endpoint("keyboard-left")?,
        target: endpoint("keyboard-right")?,
        enabled: true,
        extensions: Default::default(),
    };

    assert_ne!(binding.source, binding.target);
    assert_eq!(binding.source.port.port_id, binding.target.port.port_id);

    let json = serde_json::to_string(&binding)?;
    for transient in [
        "runtimeGroupId",
        "pluginSessionId",
        "sessionGeneration",
        "descriptorRevision",
    ] {
        assert!(!json.contains(transient), "persisted {transient}");
    }
    assert_eq!(serde_json::from_str::<EndpointBinding>(&json)?, binding);
    Ok(())
}
