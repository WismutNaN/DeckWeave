// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_protocol::{
    framing::{read_envelope, write_envelope},
    v1::{
        ActivationReply, EndpointPath, FeatureRequirement, Heartbeat, Hello, InvocationReply,
        OpaquePayload, ProtocolEnvelope, ProtocolLimits, ProtocolVersion, ProtocolVersionRange,
        SchemaReference, Signal, TerminalOutcome, endpoint_path, invocation_reply,
        protocol_envelope, terminal_outcome,
    },
};
use std::{error::Error, io, process::Command, time::Duration};
#[path = "virtual_plugin_held.rs"]
mod held;

const DESCRIPTOR: &str =
    include_str!("../../../../protocol/fixtures/descriptors/observe-keyboard.node.json");

fn main() -> Result<(), Box<dyn Error>> {
    let package_id = std::env::var("DECKWEAVE_PACKAGE_ID")?;
    let runtime_group = std::env::var("DECKWEAVE_RUNTIME_GROUP")?;
    let token = decode_hex(&std::env::var("DECKWEAVE_ONE_TIME_TOKEN")?)?;
    let generation: u64 = std::env::var("DECKWEAVE_SESSION_GENERATION")?.parse()?;
    let maximum: usize = std::env::var("DECKWEAVE_MAXIMUM_MESSAGE_BYTES")?.parse()?;
    let mode = std::env::var("DECKWEAVE_FIXTURE_MODE").unwrap_or_default();
    if mode == "grandchild_marker" {
        let marker = std::env::var("DECKWEAVE_FIXTURE_MARKER")?;
        std::thread::sleep(Duration::from_millis(500));
        std::fs::write(marker, b"orphaned")?;
        return Ok(());
    }
    if mode == "hold_inherited_pipes" {
        std::thread::sleep(Duration::from_secs(60));
        return Ok(());
    }
    eprintln!("virtual fixture started");

    if mode == "oversized" {
        use std::io::Write;
        io::stdout().write_all(&[0x80, 0x80, 0x80, 0x80, 0x10])?;
        io::stdout().flush()?;
        std::thread::sleep(Duration::from_millis(100));
        return Ok(());
    }
    if mode == "spam_stderr" {
        use std::io::Write;
        let block = [b'x'; 1024];
        for _ in 0..256 {
            io::stderr().write_all(&block)?;
        }
        io::stderr().flush()?;
    }

    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut message_id = 1_u64;
    send(
        &mut stdout,
        maximum,
        generation,
        message_id,
        protocol_envelope::Body::Hello(Hello {
            protocol: Some(ProtocolVersionRange {
                minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
                maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
            }),
            package_id,
            runtime_group,
            one_time_token: token,
            features: vec![
                FeatureRequirement {
                    id: deckweave_protocol::FEATURE_ACTIVATION_COMMIT.into(),
                    minimum_version: 1,
                    maximum_version: 1,
                    required: false,
                },
                FeatureRequirement {
                    id: deckweave_protocol::FEATURE_CONFIGURED_INSTANCES.into(),
                    minimum_version: 1,
                    maximum_version: 1,
                    required: false,
                },
                FeatureRequirement {
                    id: deckweave_protocol::FEATURE_LOCALE_CONTEXT.into(),
                    minimum_version: 1,
                    maximum_version: 1,
                    required: false,
                },
                FeatureRequirement {
                    id: deckweave_protocol::FEATURE_OWNER_CLEANUP.into(),
                    minimum_version: 1,
                    maximum_version: 1,
                    required: false,
                },
                FeatureRequirement {
                    id: deckweave_protocol::FEATURE_INVOCATION_ENDPOINT_PATH.into(),
                    minimum_version: 1,
                    maximum_version: 1,
                    required: true,
                },
                FeatureRequirement {
                    id: deckweave_protocol::FEATURE_VALUE_WRITE_ENDPOINT_PATH.into(),
                    minimum_version: 1,
                    maximum_version: 1,
                    required: true,
                },
            ],
            schemas: vec![],
            limits: Some(ProtocolLimits {
                maximum_message_bytes: u64::try_from(maximum)?,
                maximum_chunk_bytes: 64 * 1024,
                maximum_concurrent_jobs: 4,
                maximum_artifact_bytes: 1024 * 1024,
                maximum_events_per_second: 100,
            }),
        }),
    )?;
    if mode == "ignore_stdin" {
        std::thread::sleep(Duration::from_secs(60));
        return Ok(());
    }
    if mode == "exit_with_inherited_pipes" {
        Command::new(std::env::current_exe()?)
            .env("DECKWEAVE_FIXTURE_MODE", "hold_inherited_pipes")
            .spawn()?;
        return Ok(());
    }
    if mode == "spam_stdout" {
        for index in 0..64_u64 {
            message_id += 1;
            send(
                &mut stdout,
                maximum,
                generation,
                message_id,
                protocol_envelope::Body::Heartbeat(Heartbeat {
                    host_ticks: index,
                    active_jobs: 0,
                }),
            )?;
        }
        return Ok(());
    }
    if mode == "crash_after_hello" {
        return Err("intentional crash".into());
    }

    let mut pending_signals: Vec<Signal> = Vec::new();
    let mut held = held::HeldOutputs::default();
    loop {
        let Some(envelope) = read_envelope(&mut stdin, maximum)? else {
            return Ok(());
        };
        match envelope.body {
            Some(protocol_envelope::Body::ActivationCommitted(_commit)) => {
                for signal in pending_signals.drain(..) {
                    message_id += 1;
                    send(
                        &mut stdout,
                        maximum,
                        generation,
                        message_id,
                        protocol_envelope::Body::Signal(signal),
                    )?;
                }
            }
            Some(protocol_envelope::Body::LocaleContext(context)) => {
                if std::env::current_exe()?
                    .with_file_name("mixed-flood")
                    .exists()
                {
                    use std::io::Write;
                    let signals = m1_signals()?;
                    let mut burst = Vec::new();
                    for index in 0..1024 {
                        let mut signal = if index % 8 == 0 {
                            signals[0].clone()
                        } else {
                            signals[2].clone()
                        };
                        if index % 8 == 0 {
                            signal.payload = Some(json_payload(
                                "org.deckweave.schema.boolean",
                                &serde_json::json!(false),
                            )?);
                        }
                        signal.sequence = 100 + index;
                        message_id += 1;
                        send(
                            &mut burst,
                            maximum,
                            generation,
                            message_id,
                            protocol_envelope::Body::Signal(signal),
                        )?;
                    }
                    stdout.write_all(&burst)?;
                    stdout.flush()?;
                }
                if let Some(request) = held.on_locale()? {
                    message_id += 1;
                    send(
                        &mut stdout,
                        maximum,
                        generation,
                        message_id,
                        protocol_envelope::Body::JobOutcome(TerminalOutcome {
                            invocation_id: request.invocation_id,
                            result: Some(terminal_outcome::Result::Succeeded(json_payload(
                                "org.deckweave.schema.text",
                                &serde_json::json!("held complete"),
                            )?)),
                        }),
                    )?;
                }
                std::fs::write(
                    std::env::current_exe()?.with_file_name("locale.json"),
                    serde_json::to_vec(
                        &serde_json::json!({"revision":context.revision,"locale":context.requested_locale,"timeZone":context.time_zone}),
                    )?,
                )?;
            }
            Some(protocol_envelope::Body::OwnerCleanup(request)) => {
                let cleanup_mode = if request.run_id.is_none() {
                    std::fs::read_to_string(std::env::current_exe()?.with_file_name("cleanup-mode"))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                if cleanup_mode == "ignore" {
                    continue;
                }
                if cleanup_mode != "reject" {
                    held.cleanup(&request)?;
                }
                message_id += 1;
                send(
                    &mut stdout,
                    maximum,
                    generation,
                    message_id,
                    protocol_envelope::Body::OwnerCleanupReceipt(
                        deckweave_protocol::v1::OwnerCleanupReceipt {
                            cleanup_id: request.cleanup_id,
                            activation_epoch: request.activation_epoch,
                            released: cleanup_mode != "reject",
                        },
                    ),
                )?;
            }
            Some(protocol_envelope::Body::HelloAck(_)) => {}
            Some(protocol_envelope::Body::Activate(request)) => {
                if mode == "spawn_grandchild" {
                    Command::new(std::env::current_exe()?)
                        .env("DECKWEAVE_FIXTURE_MODE", "grandchild_marker")
                        .spawn()?;
                }
                message_id += 1;
                let contribution = request.contribution_id.clone();
                let mut descriptors = descriptors(&contribution)?;
                if let Some(id) = &request.configured_instance_id {
                    let mut value: serde_json::Value = serde_json::from_slice(&descriptors)?;
                    value[0]["devices"][0]["identity"] = serde_json::json!(id);
                    descriptors = serde_json::to_vec(&value)?;
                    use std::io::Write;
                    let mut log = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(std::env::current_exe()?.with_file_name("activations.jsonl"))?;
                    let settings: serde_json::Value = serde_json::from_slice(
                        &request.settings.as_ref().ok_or("settings")?.value,
                    )?;
                    writeln!(
                        log,
                        "{}",
                        serde_json::json!({"id":id,"settings":settings,"revision":request.settings_revision,"pid":std::process::id()})
                    )?;
                }
                send(
                    &mut stdout,
                    maximum,
                    generation,
                    message_id,
                    protocol_envelope::Body::Activated(ActivationReply {
                        runtime_lease_id: request.runtime_lease_id,
                        descriptor_revision: 1,
                        descriptor: Some(OpaquePayload {
                            schema: Some(SchemaReference {
                                id: "org.deckweave.descriptor.node_list".into(),
                                major: 1,
                                minor: 0,
                            }),
                            value: descriptors,
                        }),
                    }),
                )?;
                if contribution == "m1-source" {
                    pending_signals.extend(m1_signals()?);
                }
                message_id += 1;
                send(
                    &mut stdout,
                    maximum,
                    generation,
                    message_id,
                    protocol_envelope::Body::Heartbeat(Heartbeat {
                        host_ticks: 1,
                        active_jobs: 0,
                    }),
                )?;
            }
            Some(protocol_envelope::Body::Deactivate(_)) if mode == "hang_on_stop" => {
                std::thread::sleep(Duration::from_secs(60));
            }
            Some(protocol_envelope::Body::Deactivate(_)) => return Ok(()),
            Some(protocol_envelope::Body::Invoke(request)) if mode != "ignore_invocation" => {
                message_id += 1;
                send(
                    &mut stdout,
                    maximum,
                    generation,
                    message_id,
                    protocol_envelope::Body::InvocationReply(InvocationReply {
                        invocation_id: request.invocation_id.clone(),
                        reply: Some(invocation_reply::Reply::AcceptedJobId(format!(
                            "job-{}",
                            request.invocation_id
                        ))),
                    }),
                )?;
                if held.hold(&request)? {
                    continue;
                }
                message_id += 1;
                send(
                    &mut stdout,
                    maximum,
                    generation,
                    message_id,
                    protocol_envelope::Body::JobOutcome(TerminalOutcome {
                        invocation_id: request.invocation_id.clone(),
                        result: Some(terminal_outcome::Result::Succeeded(json_payload(
                            "org.deckweave.schema.text",
                            &serde_json::Value::String(format!("completed:{}", request.run_id)),
                        )?)),
                    }),
                )?;
                message_id += 1;
                send(
                    &mut stdout,
                    maximum,
                    generation,
                    message_id,
                    protocol_envelope::Body::JobOutcome(TerminalOutcome {
                        invocation_id: request.invocation_id,
                        result: Some(terminal_outcome::Result::Succeeded(json_payload(
                            "org.deckweave.schema.text",
                            &serde_json::Value::String(format!("completed:{}", request.run_id)),
                        )?)),
                    }),
                )?;
            }
            Some(protocol_envelope::Body::ValueWrite(write)) => {
                let value = write
                    .value
                    .as_ref()
                    .map(|payload| payload.value.clone())
                    .unwrap_or_default();
                use std::io::Write;
                let mut marker = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("feedback.received.json")?;
                marker.write_all(&value)?;
                marker.write_all(b"\n")?;
            }
            _ => {}
        }
    }
}

fn descriptors(contribution: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let descriptors = match contribution {
        "m1-source" => vec![m1_source_descriptor()],
        "m1-action" => vec![m1_service_descriptor(
            "m1-action-node",
            "action-service",
            "invoke",
            "action",
            "invoke",
            "CommandIn",
            "command",
            "org.deckweave.schema.unit",
        )],
        "m1-feedback" => vec![m1_service_descriptor(
            "m1-feedback-node",
            "feedback-service",
            "display",
            "text",
            "value",
            "ValueIn",
            "value",
            &feedback_schema()?,
        )],
        _ => return Ok(format!("[{DESCRIPTOR}]").into_bytes()),
    };
    Ok(serde_json::to_vec(&descriptors)?)
}

fn feedback_schema() -> Result<String, Box<dyn Error>> {
    let path = std::env::current_exe()?.with_file_name("feedback-schema");
    if !path.exists() {
        return Ok("org.deckweave.schema.text".into());
    }
    let bytes = std::fs::read(path)?;
    if bytes.len() > 256 {
        return Err("fixture schema limit".into());
    }
    Ok(String::from_utf8(bytes)?)
}

fn m1_source_descriptor() -> serde_json::Value {
    let elements = serde_json::json!([
        element(
            "button",
            "button",
            "pressed",
            "EventOut",
            "event",
            "org.deckweave.schema.boolean"
        ),
        element(
            "axis",
            "axis",
            "value",
            "ValueOut",
            "value",
            "org.deckweave.schema.number"
        ),
        element(
            "state",
            "state",
            "value",
            "ValueOut",
            "value",
            "org.deckweave.schema.text"
        )
    ]);
    serde_json::json!({
        "nodeId": "m1-source-node",
        "kind": "virtual",
        "label": "M1 source",
        "descriptorRevision": 1,
        "devices": [
            { "identity": "device-a", "providerId": "m1-provider", "kind": "virtual", "label": "Twin A", "evidence": [], "elements": elements },
            { "identity": "device-b", "providerId": "m1-provider", "kind": "virtual", "label": "Twin B", "evidence": [], "elements": elements }
        ]
    })
}

#[allow(clippy::too_many_arguments)]
fn m1_service_descriptor(
    node: &str,
    service: &str,
    element_id: &str,
    capability_id: &str,
    port_id: &str,
    kind: &str,
    semantics: &str,
    schema: &str,
) -> serde_json::Value {
    serde_json::json!({
        "nodeId": node,
        "kind": "virtual",
        "label": node,
        "descriptorRevision": 1,
        "services": [{
            "serviceId": service,
            "providerId": "m1-provider",
            "label": service,
            "elements": [element(element_id, capability_id, port_id, kind, semantics, schema)]
        }]
    })
}

fn element(
    element_id: &str,
    capability_id: &str,
    port_id: &str,
    kind: &str,
    semantics: &str,
    schema: &str,
) -> serde_json::Value {
    serde_json::json!({
        "elementId": element_id,
        "label": element_id,
        "capabilities": [{
            "capabilityId": capability_id,
            "kind": format!("org.deckweave.fixture.{capability_id}"),
            "ports": [{
                "portId": port_id,
                "kind": kind,
                "direction": if kind.ends_with("Out") { "output" } else { "input" },
                "semantics": semantics,
                "schema": { "id": schema, "version": { "major": 1, "minor": 0 } },
                "compatibleVersions": {
                    "minimum": { "major": 1, "minor": 0 },
                    "maximum": { "major": 1, "minor": 0 }
                },
                "availability": { "status": "available" },
                "lifetime": "always",
                "descriptorRevision": 1,
                "tags": ["m1"]
            }]
        }]
    })
}

#[allow(deprecated)]
fn m1_signals() -> Result<Vec<Signal>, Box<dyn Error>> {
    [
        (
            "button",
            "button",
            "pressed",
            "org.deckweave.schema.boolean",
            serde_json::json!(true),
        ),
        (
            "button",
            "button",
            "pressed",
            "org.deckweave.schema.boolean",
            serde_json::json!(true),
        ),
        (
            "axis",
            "axis",
            "value",
            "org.deckweave.schema.number",
            serde_json::json!(0.5),
        ),
        (
            "state",
            "state",
            "value",
            "org.deckweave.schema.text",
            serde_json::json!("ready"),
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (element, capability, port, schema, value))| {
        Ok(Signal {
            source_port_id: String::new(),
            device_session_generation: 1,
            sequence: u64::try_from(index + 1)?,
            captured_ticks: u64::try_from(index + 1)? * 10,
            clock_domain_id: "fixture-clock".into(),
            wall_clock_rfc3339: None,
            payload: Some(json_payload(schema, &value)?),
            source: Some(EndpointPath {
                node_id: "m1-source-node".into(),
                provider_id: "m1-provider".into(),
                target: Some(endpoint_path::Target::DeviceIdentity("device-b".into())),
                element_id: element.into(),
                capability_id: capability.into(),
                port_id: port.into(),
            }),
        })
    })
    .collect()
}

fn json_payload(schema: &str, value: &serde_json::Value) -> Result<OpaquePayload, Box<dyn Error>> {
    Ok(OpaquePayload {
        schema: Some(SchemaReference {
            id: schema.into(),
            major: 1,
            minor: 0,
        }),
        value: serde_json::to_vec(value)?,
    })
}

fn send(
    writer: &mut impl io::Write,
    maximum: usize,
    generation: u64,
    message_id: u64,
    body: protocol_envelope::Body,
) -> Result<(), Box<dyn Error>> {
    write_envelope(
        writer,
        &ProtocolEnvelope {
            wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
            session_generation: generation,
            message_id,
            trace: None,
            body: Some(body),
        },
        maximum,
    )?;
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !value.len().is_multiple_of(2) {
        return Err("odd token length".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
        .collect()
}
