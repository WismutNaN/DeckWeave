// SPDX-License-Identifier: GPL-3.0-or-later
use super::runtime::*;
use deckweave_protocol::v1::*;
use serde_json::json;
use std::{collections::BTreeMap, error::Error};
fn request(text: &str) -> Result<InvocationRequest, Box<dyn Error>> {
    Ok(InvocationRequest {
        invocation_id: "invocation".into(),
        activation_epoch: 1,
        descriptor_revision: 1,
        operation_id: "echo".into(),
        configured_instance_id: Some("home".into()),
        target: Some(EndpointPath {
            node_id: "virtual-node".into(),
            provider_id: "virtual".into(),
            target: Some(endpoint_path::Target::ServiceId("text".into())),
            element_id: "echo".into(),
            capability_id: "action".into(),
            port_id: "invoke".into(),
        }),
        input: Some(OpaquePayload {
            schema: Some(SchemaReference {
                id: "org.deckweave.virtual.message".into(),
                major: 1,
                minor: 0,
            }),
            value: serde_json::to_vec(&json!({"text":text}))?,
        }),
        ..Default::default()
    })
}
#[test]
fn accounts_never_fall_back_and_stale_epochs_are_rejected() -> Result<(), Box<dyn Error>> {
    let accounts = BTreeMap::from([
        (Some("home".into()), "H:".into()),
        (None, "default:".into()),
    ]);
    let mut r = request("Hello")?;
    assert_eq!(invoke(&r, Some(1), &accounts), Ok("H:Hello".into()));
    r.configured_instance_id = Some("missing".into());
    assert_eq!(invoke(&r, Some(1), &accounts), Err("unknown_instance"));
    r.configured_instance_id = None;
    assert_eq!(invoke(&r, Some(2), &accounts), Err("invalid_target"));
    Ok(())
}
#[test]
fn result_respects_wire_schema_at_utf8_boundaries() -> Result<(), Box<dyn Error>> {
    let accounts = BTreeMap::from([(Some("home".into()), "Ж".repeat(39))]);
    let result =
        invoke(&request(&"🦫".repeat(512))?, Some(1), &accounts).map_err(std::io::Error::other)?;
    assert!(result.len() <= 2048);
    assert!(result.ends_with('🦫'));
    assert_eq!(
        invoke(&request(&"🦫".repeat(513))?, Some(1), &accounts),
        Err("invalid_text")
    );
    Ok(())
}
