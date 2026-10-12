// SPDX-License-Identifier: GPL-3.0-or-later
use crate::wire::WireSender;
use deckweave_protocol::{framing::read_envelope, v1::*, *};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
const DESCRIPTORS: &[u8] = include_bytes!("../assets/descriptors.json");
const PACKAGE: &str = "org.deckweave.virtual";

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    if std::env::var("DECKWEAVE_PACKAGE_ID")? != PACKAGE
        || std::env::var("DECKWEAVE_RUNTIME_GROUP")? != "control"
    {
        return Err("incorrect runtime identity".into());
    }
    let generation = std::env::var("DECKWEAVE_SESSION_GENERATION")?.parse()?;
    let maximum = std::env::var("DECKWEAVE_MAXIMUM_MESSAGE_BYTES")?.parse()?;
    let token = std::env::var("DECKWEAVE_ONE_TIME_TOKEN")?;
    if !token.is_ascii() || !token.len().is_multiple_of(2) {
        return Err("invalid token".into());
    }
    let token = (0..token.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&token[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()?;
    let (wire, writer) = WireSender::start(generation, maximum)?;
    wire.send(protocol_envelope::Body::Hello(Hello {
        protocol: Some(ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        }),
        package_id: PACKAGE.into(),
        runtime_group: "control".into(),
        one_time_token: token,
        features: [
            FEATURE_ACTIVATION_COMMIT,
            FEATURE_INVOCATION_ENDPOINT_PATH,
            FEATURE_VALUE_WRITE_ENDPOINT_PATH,
            FEATURE_CONFIGURED_INSTANCES,
            FEATURE_CONFIGURED_DISPATCH,
            FEATURE_OWNER_CLEANUP,
            FEATURE_LOCALE_CONTEXT,
        ]
        .into_iter()
        .map(|id| FeatureRequirement {
            id: id.into(),
            minimum_version: 1,
            maximum_version: 1,
            required: true,
        })
        .collect(),
        schemas: vec![],
        limits: Some(ProtocolLimits {
            maximum_message_bytes: u64::try_from(maximum)?,
            maximum_chunk_bytes: 65536,
            maximum_concurrent_jobs: 16,
            maximum_artifact_bytes: 1048576,
            maximum_events_per_second: 512,
        }),
    }))?;
    let stopped = Arc::new(AtomicBool::new(false));
    let heartbeat = {
        let stopped = stopped.clone();
        let wire = wire.clone();
        std::thread::spawn(move || {
            let mut ticks = 0;
            while !stopped.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(50));
                ticks += 50;
                if ticks % 1000 == 0
                    && wire
                        .send(protocol_envelope::Body::Heartbeat(Heartbeat {
                            host_ticks: ticks,
                            active_jobs: 0,
                        }))
                        .is_err()
                {
                    break;
                }
            }
        })
    };
    let result = serve(&wire, generation);
    stopped.store(true, Ordering::Release);
    let _ = heartbeat.join();
    drop(wire);
    let finished = writer.join().map_err(|_| "writer panicked")?;
    result?;
    finished.map_err(Into::into)
}

fn serve(wire: &WireSender, generation: u64) -> Result<(), Box<dyn Error>> {
    let mut accounts = BTreeMap::new();
    let mut epoch = None;
    let mut devices = false;
    let mut sequence = 0;
    let mut stdin = io::stdin().lock();
    while let Some(message) = read_envelope(&mut stdin, wire.maximum_message_bytes())? {
        if message.session_generation != generation {
            return Err("stale generation".into());
        }
        match message.body {
            Some(protocol_envelope::Body::HelloAck(ack)) => wire.set_maximum_message_bytes(
                usize::try_from(ack.limits.ok_or("limits")?.maximum_message_bytes)?,
            ),
            Some(protocol_envelope::Body::Activate(request)) => {
                if !["devices", "text"].contains(&request.contribution_id.as_str()) {
                    return Err("contribution".into());
                }
                let mut nodes: Value = serde_json::from_slice(DESCRIPTORS)?;
                if request.contribution_id == "devices" {
                    if request.configured_instance_id.is_some() {
                        return Err("devices cannot be configured".into());
                    }
                    devices = true;
                    nodes[0]["services"] = json!([]);
                } else {
                    nodes[0]["devices"] = json!([]);
                    let settings: Value = request
                        .settings
                        .map(|s| serde_json::from_slice(&s.value))
                        .transpose()?
                        .unwrap_or(json!({"prefix":""}));
                    let prefix = settings["prefix"]
                        .as_str()
                        .filter(|p| p.len() <= 80)
                        .ok_or("prefix")?;
                    if accounts.len() >= 64 {
                        return Err("account limit".into());
                    }
                    accounts.insert(request.configured_instance_id, prefix.to_owned());
                }
                wire.send(protocol_envelope::Body::Activated(ActivationReply {
                    runtime_lease_id: request.runtime_lease_id,
                    descriptor_revision: 1,
                    descriptor: Some(payload("org.deckweave.descriptor.node_list", &nodes)?),
                }))?;
            }
            Some(protocol_envelope::Body::ActivationCommitted(commit)) => {
                epoch = Some(commit.activation_epoch)
            }
            Some(protocol_envelope::Body::Invoke(request)) => {
                let result = invoke(&request, epoch, &accounts);
                let reply = match result {
                    Ok(text) => invocation_reply::Reply::Terminal(TerminalOutcome {
                        invocation_id: request.invocation_id.clone(),
                        result: Some(terminal_outcome::Result::Succeeded(payload(
                            "org.deckweave.virtual.text",
                            &json!(text),
                        )?)),
                    }),
                    Err(code) => invocation_reply::Reply::Rejected(StructuredError {
                        category: ErrorCategory::InvalidInput as i32,
                        code: code.into(),
                        message_key: "virtual.invalid_request".into(),
                        details: Default::default(),
                        diagnostic_attachment_id: None,
                    }),
                };
                wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
                    invocation_id: request.invocation_id,
                    reply: Some(reply),
                }))?;
            }
            Some(protocol_envelope::Body::ValueWrite(write)) => {
                if !devices
                    || epoch != Some(write.activation_epoch)
                    || write.configured_instance_id.is_some()
                {
                    return Err("invalid display owner".into());
                }
                let mut target = write.target.ok_or("display target")?;
                if !device_target(&target)
                    || target.capability_id != "display"
                    || target.port_id != "text"
                {
                    return Err("invalid display endpoint".into());
                }
                let value = write.value.ok_or("display value")?;
                let text: String = serde_json::from_slice(&value.value)?;
                if text.len() > 2048
                    || value
                        .schema
                        .as_ref()
                        .is_none_or(|s| s.id != "org.deckweave.virtual.text" || s.major != 1)
                {
                    return Err("display schema".into());
                }
                target.port_id = "displayed".into();
                sequence += 1;
                #[allow(deprecated)]
                let signal = Signal {
                    source_port_id: String::new(),
                    source: Some(target),
                    device_session_generation: generation,
                    sequence,
                    captured_ticks: sequence,
                    clock_domain_id: "virtual-display".into(),
                    wall_clock_rfc3339: None,
                    payload: Some(value),
                };
                wire.send(protocol_envelope::Body::Signal(signal))?;
            }
            Some(protocol_envelope::Body::OwnerCleanup(request)) => wire.send(
                protocol_envelope::Body::OwnerCleanupReceipt(OwnerCleanupReceipt {
                    cleanup_id: request.cleanup_id,
                    activation_epoch: request.activation_epoch,
                    released: true,
                }),
            )?,
            Some(protocol_envelope::Body::Deactivate(_)) => return Ok(()),
            _ => {}
        }
    }
    Ok(())
}
pub(crate) fn invoke(
    request: &InvocationRequest,
    epoch: Option<u64>,
    accounts: &BTreeMap<Option<String>, String>,
) -> Result<String, &'static str> {
    let target = request.target.as_ref().ok_or("missing_target")?;
    if epoch != Some(request.activation_epoch)
        || request.descriptor_revision != 1
        || request.operation_id != "echo"
        || target.node_id != "virtual-node"
        || target.provider_id != "virtual"
        || target.element_id != "echo"
        || target.capability_id != "action"
        || target.port_id != "invoke"
        || target.target != Some(endpoint_path::Target::ServiceId("text".into()))
    {
        return Err("invalid_target");
    }
    let prefix = accounts
        .get(&request.configured_instance_id)
        .ok_or("unknown_instance")?;
    let input = request.input.as_ref().ok_or("missing_input")?;
    if input
        .schema
        .as_ref()
        .is_none_or(|s| s.id != "org.deckweave.virtual.message" || s.major != 1)
    {
        return Err("invalid_schema");
    }
    let value: Value = serde_json::from_slice(&input.value).map_err(|_| "invalid_json")?;
    let text = value["text"]
        .as_str()
        .filter(|s| s.len() <= 2048)
        .ok_or("invalid_text")?;
    let mut result = format!("{prefix}{text}");
    let mut end = result.len().min(2048);
    while !result.is_char_boundary(end) {
        end -= 1;
    }
    result.truncate(end);
    Ok(result)
}
fn device_target(target: &EndpointPath) -> bool {
    target.node_id == "virtual-node"
        && target.provider_id == "virtual"
        && matches!(&target.target, Some(endpoint_path::Target::DeviceIdentity(id)) if id == "deck-a" || id == "deck-b")
        && target
            .element_id
            .strip_prefix("key-")
            .and_then(|s| s.parse::<usize>().ok())
            .is_some_and(|i| (1..=15).contains(&i))
}
fn payload(id: &str, value: &Value) -> Result<OpaquePayload, serde_json::Error> {
    Ok(OpaquePayload {
        schema: Some(SchemaReference {
            id: id.into(),
            major: 1,
            minor: 0,
        }),
        value: serde_json::to_vec(value)?,
    })
}
