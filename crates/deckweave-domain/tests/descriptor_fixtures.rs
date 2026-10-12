// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{
    CapabilityId, DeviceIdentity, ElementId, NodeId, PackageId, PortId, ProviderId, ServiceId,
};
use deckweave_domain::{
    automation::{ExecutionKind, OperationDescriptor},
    capability::{NodeDescriptor, SignalEnvelope, SignalValidationError},
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
    lighting::LightTopology,
    media::StreamTemplate,
    remote::RemoteNodeBoundary,
    visual::DisplaySurfaceDescriptor,
};
use proptest::prelude::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{error::Error, fs, path::PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/fixtures/descriptors")
        .join(name)
}

fn fixture_text(name: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(fixture_path(name))?)
}

fn fixture<T: DeserializeOwned>(name: &str) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_str(&fixture_text(name)?)?)
}

fn stable_round_trip<T>(name: &str) -> Result<(), Box<dyn Error>>
where
    T: DeserializeOwned + Serialize,
{
    let first: T = fixture(name)?;
    let serialized = serde_json::to_string(&first)?;
    let second: T = serde_json::from_str(&serialized)?;
    assert_eq!(serialized, serde_json::to_string(&second)?);
    Ok(())
}

fn port_ids(node: &NodeDescriptor) -> Vec<&str> {
    node.devices
        .iter()
        .flat_map(|device| &device.elements)
        .chain(node.services.iter().flat_map(|service| &service.elements))
        .flat_map(|element| &element.capabilities)
        .flat_map(|capability| &capability.ports)
        .map(|port| port.port_id.as_str())
        .collect()
}

#[test]
fn stable_endpoint_json_excludes_runtime_placement() -> Result<(), Box<dyn Error>> {
    let endpoint = StableEndpointRef {
        package_id: PackageId::try_new("org.deckweave.keyboard")?,
        node_id: NodeId::try_new("local")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("hid")?,
            device_identity: DeviceIdentity::try_new("keyboard-1")?,
        },
        port: PortPath {
            element_id: ElementId::try_new("key-a")?,
            capability_id: CapabilityId::try_new("button")?,
            port_id: PortId::try_new("pressed")?,
        },
    };

    assert_eq!(
        serde_json::to_value(endpoint)?,
        json!({
            "packageId": "org.deckweave.keyboard",
            "nodeId": "local",
            "target": {
                "kind": "device",
                "providerId": "hid",
                "deviceIdentity": "keyboard-1"
            },
            "port": {
                "elementId": "key-a",
                "capabilityId": "button",
                "portId": "pressed"
            }
        })
    );
    Ok(())
}

#[test]
fn observe_keyboard_does_not_invent_analog_travel() -> Result<(), Box<dyn Error>> {
    let node: NodeDescriptor = fixture("observe-keyboard.node.json")?;
    node.validate()?;
    assert_eq!(port_ids(&node), ["key-a.edge"]);
    assert_eq!(
        node.devices[0].extensions["org.deckweave.fixture.futureDeviceField"]["preserved"],
        true
    );
    stable_round_trip::<NodeDescriptor>("observe-keyboard.node.json")?;
    Ok(())
}

#[test]
fn port_ids_are_scoped_by_capability_path() -> Result<(), Box<dyn Error>> {
    let mut node: NodeDescriptor = fixture("observe-keyboard.node.json")?;
    let mut second_capability = node.devices[0].elements[0].capabilities[0].clone();
    second_capability.capability_id = CapabilityId::try_new("key-a-secondary")?;
    node.devices[0].elements[0]
        .capabilities
        .push(second_capability);
    node.validate()?;
    assert_eq!(port_ids(&node), ["key-a.edge", "key-a.edge"]);
    Ok(())
}

#[test]
fn analog_keyboard_declares_measured_travel_explicitly() -> Result<(), Box<dyn Error>> {
    let node: NodeDescriptor = fixture("analog-keyboard.node.json")?;
    node.validate()?;
    assert_eq!(port_ids(&node), ["key-w.travel"]);
    let range = node.devices[0].elements[0].capabilities[0].ports[0]
        .range
        .as_ref()
        .ok_or("travel must have range")?;
    assert_eq!((range.minimum, range.maximum), (0.0, 1.0));
    Ok(())
}

#[test]
fn irregular_lighting_supports_holes_many_to_many_and_shared_leds() -> Result<(), Box<dyn Error>> {
    let topology: LightTopology = fixture("irregular-rgb.topology.json")?;
    topology.validate()?;
    let mappings = &topology.element_mappings;
    assert!(
        mappings
            .iter()
            .any(|mapping| mapping.light_unit_ids.is_empty())
    );
    assert!(
        mappings
            .iter()
            .any(|mapping| mapping.light_unit_ids.len() == 2)
    );
    assert_eq!(
        mappings
            .iter()
            .filter(|mapping| {
                mapping
                    .light_unit_ids
                    .iter()
                    .any(|id| id.as_str() == "led-shared")
            })
            .count(),
        2
    );
    stable_round_trip::<LightTopology>("irregular-rgb.topology.json")?;
    Ok(())
}

#[test]
fn screen_frame_and_pointer_are_independent_capabilities() -> Result<(), Box<dyn Error>> {
    let node: NodeDescriptor = fixture("screen-pointer.node.json")?;
    node.validate()?;
    assert_eq!(port_ids(&node), ["surface.frame", "surface.pointer"]);

    let surface: DisplaySurfaceDescriptor = fixture("screen-pointer.surface.json")?;
    surface.validate()?;
    assert!(surface.full_frame && surface.partial_update && surface.atlas);
    assert_eq!(
        surface.extensions["org.deckweave.fixture.hitTest"],
        "coordinate-sample-only"
    );
    Ok(())
}

#[test]
fn async_service_uses_a_typed_job_operation() -> Result<(), Box<dyn Error>> {
    let node: NodeDescriptor = fixture("async-service.node.json")?;
    node.validate()?;
    assert_eq!(port_ids(&node), ["transcribe.invoke"]);

    let operation: OperationDescriptor = fixture("async-service.operation.json")?;
    operation.validate()?;
    assert_eq!(operation.execution.kind, ExecutionKind::Job);
    assert!(operation.execution.progress && operation.execution.cancellation);
    Ok(())
}

#[test]
fn media_and_remote_are_descriptors_not_a_premature_buffer_abi() -> Result<(), Box<dyn Error>> {
    let stream: StreamTemplate = fixture("media.stream-template.json")?;
    stream.validate()?;
    assert_eq!(
        stream.extensions["org.deckweave.stream.dataPlane"],
        "not-selected"
    );

    let remote: RemoteNodeBoundary = fixture("remote-node.boundary.json")?;
    remote.validate()?;
    assert_eq!(remote.channels.len(), 2);
    assert_eq!(
        remote.channels[1].extensions["org.deckweave.stream.bufferAbi"],
        "intentionally-undefined"
    );
    Ok(())
}

#[test]
fn signal_sequence_and_host_ingress_are_not_derived_from_value_direction()
-> Result<(), Box<dyn Error>> {
    let deltas = [1, 1, -1];
    let signals: Vec<_> = deltas
        .into_iter()
        .enumerate()
        .map(|(index, delta)| {
            serde_json::from_value::<SignalEnvelope>(json!({
                "source": {
                    "stable": {
                        "packageId": "org.deckweave.fixture.keyboard",
                        "nodeId": "local-node",
                        "target": {
                            "kind": "device",
                            "providerId": "fixture-hid-provider",
                            "deviceIdentity": "keyboard-observe-fixture"
                        },
                        "port": {
                            "elementId": "encoder",
                            "capabilityId": "encoder-rotation",
                            "portId": "encoder.delta"
                        }
                    },
                    "runtimeGroupId": "input",
                    "pluginSessionId": "plugin-session-1",
                    "sessionGeneration": 1,
                    "descriptorRevision": 1
                },
                "deviceSessionGeneration": 1,
                "sequence": index + 1,
                "capturedAt": {
                    "ticks": index + 100,
                    "frequencyHz": 1000,
                    "clockDomainId": "device-clock"
                },
                "hostIngress": {
                    "receivedTicks": 500 + index,
                    "ingressOrdinal": index + 1,
                    "hostBootId": "host-boot-1",
                    "activationEpoch": 1
                },
                "quality": "fresh",
                "origin": "physical",
                "trace": { "traceId": "trace-1" },
                "payload": { "delta": delta }
            }))
        })
        .collect::<Result<_, _>>()?;
    for signal in &signals {
        signal.validate()?;
    }
    let decoded_deltas: Vec<i64> = signals
        .iter()
        .map(|signal| signal.payload["delta"].as_i64())
        .collect::<Option<_>>()
        .ok_or("all deltas must be numeric")?;
    assert_eq!(decoded_deltas, [1, 1, -1]);

    let mut missing_generation = signals[0].clone();
    missing_generation.device_session_generation = None;
    assert_eq!(
        missing_generation.validate(),
        Err(SignalValidationError::MissingDeviceGeneration)
    );

    let mut service_signal = signals[0].clone();
    service_signal.source.stable.target = EndpointTargetRef::Service {
        provider_id: ProviderId::try_new("fixture-service-provider")?,
        service_id: ServiceId::try_new("keyboard-service")?,
    };
    assert_eq!(
        service_signal.validate(),
        Err(SignalValidationError::UnexpectedDeviceGeneration)
    );
    Ok(())
}

proptest! {
    #[test]
    fn inverted_or_zero_precision_ranges_are_rejected(
        minimum in -1000.0f64..1000.0,
        gap in 0.0001f64..1000.0,
        non_positive_precision in -1000.0f64..=0.0,
    ) {
        let source = fixture_text("analog-keyboard.node.json")
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let mut inverted: Value = serde_json::from_str(&source)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let range = &mut inverted["devices"][0]["elements"][0]["capabilities"][0]["ports"][0]["range"];
        range["minimum"] = json!(minimum + gap);
        range["maximum"] = json!(minimum);
        let node: NodeDescriptor = serde_json::from_value(inverted)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert!(node.validate().is_err());

        let mut precision: Value = serde_json::from_str(&source)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        precision["devices"][0]["elements"][0]["capabilities"][0]["ports"][0]["range"]["precision"] = json!(non_positive_precision);
        let node: NodeDescriptor = serde_json::from_value(precision)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert!(node.validate().is_err());
    }

    #[test]
    fn unknown_optional_device_fields_survive_json_round_trip(
        suffix in "[A-Za-z][A-Za-z0-9]{0,16}",
        value in any::<i64>(),
    ) {
        let key = format!("org.deckweave.fixture.{suffix}");
        let source = fixture_text("observe-keyboard.node.json")
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let mut document: Value = serde_json::from_str(&source)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        document["devices"][0][&key] = json!(value);
        let typed: NodeDescriptor = serde_json::from_value(document)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let round_trip = serde_json::to_value(typed)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(round_trip["devices"][0][&key].as_i64(), Some(value));
    }
}
