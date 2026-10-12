// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_protocol::{
    framing::{read_envelope, write_envelope},
    v1::{
        ActivationReply, FeatureRequirement, Heartbeat, Hello, OpaquePayload, ProtocolEnvelope,
        ProtocolLimits, ProtocolVersion, ProtocolVersionRange, SchemaReference, protocol_envelope,
    },
};
use std::{error::Error, io, process::Command, time::Duration};

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

    loop {
        let Some(envelope) = read_envelope(&mut stdin, maximum)? else {
            return Ok(());
        };
        match envelope.body {
            Some(protocol_envelope::Body::HelloAck(_)) => {}
            Some(protocol_envelope::Body::Activate(request)) => {
                if mode == "spawn_grandchild" {
                    Command::new(std::env::current_exe()?)
                        .env("DECKWEAVE_FIXTURE_MODE", "grandchild_marker")
                        .spawn()?;
                }
                message_id += 1;
                let descriptors = format!("[{DESCRIPTOR}]").into_bytes();
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
            _ => {}
        }
    }
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
