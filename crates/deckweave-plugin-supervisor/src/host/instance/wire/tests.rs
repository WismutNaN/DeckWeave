// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn wire_qos_does_not_coalesce_edges_or_relative_values() -> Result<(), Box<dyn std::error::Error>> {
    use deckweave_domain::{
        capability::{NodeDescriptor, PortSemantics},
        ids::*,
    };
    let mut nodes: Vec<NodeDescriptor> = vec![serde_json::from_str(include_str!(
        "../../../../../../protocol/fixtures/descriptors/observe-keyboard.node.json"
    ))?];
    let session = SessionIdentity {
        package_id: PackageId::try_new("org.fixture.keys")?,
        runtime_group_id: RuntimeGroupId::try_new("input")?,
        plugin_session_id: PluginSessionId::try_new("session")?,
        generation: SessionGeneration::try_new(1)?,
    };
    let epoch = ActivationEpoch::try_new(1)?;
    let source = v1::EndpointPath {
        node_id: "local-node".into(),
        provider_id: "fixture-hid-provider".into(),
        target: Some(endpoint_path::Target::DeviceIdentity(
            "keyboard-observe-fixture".into(),
        )),
        element_id: "key-a".into(),
        capability_id: "key-a-button".into(),
        port_id: "key-a.edge".into(),
    };
    for (semantics, latest, expected) in [
        (PortSemantics::Event, false, IngressReliability::Reliable),
        (PortSemantics::Value, false, IngressReliability::Reliable),
        (PortSemantics::State, false, IngressReliability::Coalescible),
        (PortSemantics::Value, true, IngressReliability::Coalescible),
    ] {
        let port = &mut nodes[0].devices[0].elements[0].capabilities[0].ports[0];
        port.semantics = semantics;
        port.tags.clear();
        if latest {
            port.tags.insert("org.deckweave.delivery.latest".into());
        }
        let body = protocol_envelope::Body::Signal(v1::Signal {
            source: Some(source.clone()),
            payload: Some(payload(&port.schema, &Value::Bool(true))?),
            ..Default::default()
        });
        let received = event(
            &session,
            DescriptorRevision::try_new(1)?,
            epoch,
            &body,
            &nodes,
            &mut ReplyContracts::default(),
        )?
        .ok_or("event")?;
        assert!(
            matches!(received.kind, RuntimeEventKind::Signal { reliability, .. } if reliability == expected)
        );
    }
    Ok(())
}
#[test]
fn schema_identity_and_encoding_are_checked_before_decoding()
-> Result<(), Box<dyn std::error::Error>> {
    let schema = SchemaRef {
        id: NamespacedKind::try_new("org.deckweave.number")?,
        version: SchemaVersion { major: 1, minor: 0 },
    };
    let mut wire = payload(&schema, &serde_json::json!(0.8))?;
    assert_eq!(
        decode_payload(Some(&wire), &schema)?,
        serde_json::json!(0.8)
    );
    wire.schema.as_mut().ok_or("schema")?.minor = 1;
    assert!(decode_payload(Some(&wire), &schema).is_err());
    wire.schema = None;
    assert!(decode_payload(Some(&wire), &schema).is_err());
    let mut wire = payload(&schema, &Value::Null)?;
    wire.value = vec![0xff, 0xfe];
    assert!(decode_payload(Some(&wire), &schema).is_err());
    Ok(())
}
#[test]
fn nested_terminal_cannot_reply_for_another_invocation() -> Result<(), Box<dyn std::error::Error>> {
    let schema = SchemaRef {
        id: NamespacedKind::try_new("org.deckweave.number")?,
        version: SchemaVersion { major: 1, minor: 0 },
    };
    let reply = v1::InvocationReply {
        invocation_id: "inv-a".into(),
        reply: Some(invocation_reply::Reply::Terminal(TerminalOutcome {
            invocation_id: "inv-b".into(),
            result: Some(terminal_outcome::Result::Succeeded(payload(
                &schema,
                &Value::Null,
            )?)),
        })),
    };
    assert!(invocation_reply(&reply, &schema).is_err());
    Ok(())
}
