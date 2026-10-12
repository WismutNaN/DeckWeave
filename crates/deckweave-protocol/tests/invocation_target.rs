// SPDX-License-Identifier: MIT OR Apache-2.0

use deckweave_protocol::v1;
use prost::Message;

#[test]
fn invocation_wire_request_carries_the_full_endpoint_path() -> Result<(), Box<dyn std::error::Error>>
{
    let request = v1::InvocationRequest {
        invocation_id: "invocation-1".into(),
        run_id: "run-1".into(),
        operation_id: "set-level".into(),
        activation_epoch: 7,
        descriptor_revision: 11,
        deadline_host_ticks: 100,
        idempotency_key: None,
        input: None,
        target: Some(v1::EndpointPath {
            node_id: "node-a".into(),
            provider_id: "provider-a".into(),
            target: Some(v1::endpoint_path::Target::DeviceIdentity("device-a".into())),
            element_id: "dial-1".into(),
            capability_id: "haptics".into(),
            port_id: "pulse".into(),
        }),
    };
    let decoded = v1::InvocationRequest::decode(request.encode_to_vec().as_slice())?;
    let target = decoded.target.ok_or("missing invocation target")?;
    assert_eq!(target.node_id, "node-a");
    assert_eq!(target.provider_id, "provider-a");
    assert_eq!(target.element_id, "dial-1");
    assert_eq!(target.capability_id, "haptics");
    assert_eq!(target.port_id, "pulse");
    assert!(matches!(
        target.target,
        Some(v1::endpoint_path::Target::DeviceIdentity(value)) if value == "device-a"
    ));
    Ok(())
}
