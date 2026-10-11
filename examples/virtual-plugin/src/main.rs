// SPDX-License-Identifier: MIT OR Apache-2.0
use deckweave_protocol::{
    framing::{read_envelope, write_envelope},
    v1::{
        ActivationReply, Heartbeat, Hello, InvocationReply, OpaquePayload, ProtocolEnvelope,
        ProtocolLimits, ProtocolVersion, ProtocolVersionRange, SchemaReference, TerminalOutcome,
        invocation_reply, protocol_envelope, terminal_outcome,
    },
};
use std::{error::Error, io};

const DESCRIPTORS: &[u8] = include_bytes!("../assets/descriptors.json");

struct Runtime {
    generation: u64,
    maximum_message_bytes: usize,
    next_message_id: u64,
}

impl Runtime {
    fn send(&mut self, body: protocol_envelope::Body) -> Result<(), Box<dyn Error>> {
        self.next_message_id = self
            .next_message_id
            .checked_add(1)
            .ok_or("message ID exhausted")?;
        write_envelope(
            &mut io::stdout().lock(),
            &ProtocolEnvelope {
                wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
                session_generation: self.generation,
                message_id: self.next_message_id,
                trace: None,
                body: Some(body),
            },
            self.maximum_message_bytes,
        )?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let package_id = std::env::var("DECKWEAVE_PACKAGE_ID")?;
    let runtime_group = std::env::var("DECKWEAVE_RUNTIME_GROUP")?;
    let token = decode_hex(&std::env::var("DECKWEAVE_ONE_TIME_TOKEN")?)?;
    let generation = std::env::var("DECKWEAVE_SESSION_GENERATION")?.parse()?;
    let maximum_message_bytes = std::env::var("DECKWEAVE_MAXIMUM_MESSAGE_BYTES")?.parse()?;
    let mut runtime = Runtime {
        generation,
        maximum_message_bytes,
        next_message_id: 0,
    };
    runtime.send(protocol_envelope::Body::Hello(Hello {
        protocol: Some(ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        }),
        package_id,
        runtime_group,
        one_time_token: token,
        features: vec![],
        schemas: vec![],
        limits: Some(ProtocolLimits {
            maximum_message_bytes: u64::try_from(maximum_message_bytes)?,
            maximum_chunk_bytes: 64 * 1024,
            maximum_concurrent_jobs: 4,
            maximum_artifact_bytes: 1024 * 1024,
            maximum_events_per_second: 100,
        }),
    }))?;

    let mut stdin = io::stdin().lock();
    while let Some(envelope) = read_envelope(&mut stdin, runtime.maximum_message_bytes)? {
        match envelope.body {
            Some(protocol_envelope::Body::HelloAck(ack)) => {
                let limit = ack
                    .limits
                    .map(|limits| limits.maximum_message_bytes)
                    .ok_or("HelloAck has no limits")?;
                runtime.maximum_message_bytes = usize::try_from(limit)?;
            }
            Some(protocol_envelope::Body::Activate(request)) => {
                runtime.send(protocol_envelope::Body::Activated(ActivationReply {
                    runtime_lease_id: request.runtime_lease_id,
                    descriptor_revision: 1,
                    descriptor: Some(OpaquePayload {
                        schema: Some(SchemaReference {
                            id: "org.deckweave.descriptor.node_list".into(),
                            major: 1,
                            minor: 0,
                        }),
                        value: DESCRIPTORS.to_vec(),
                    }),
                }))?;
                runtime.send(protocol_envelope::Body::Heartbeat(Heartbeat {
                    host_ticks: 0,
                    active_jobs: 0,
                }))?;
            }
            Some(protocol_envelope::Body::Invoke(request)) => {
                let result = OpaquePayload {
                    schema: Some(SchemaReference {
                        id: "org.deckweave.result.echo".into(),
                        major: 1,
                        minor: 0,
                    }),
                    value: request.input.map_or_else(Vec::new, |input| input.value),
                };
                runtime.send(protocol_envelope::Body::InvocationReply(InvocationReply {
                    invocation_id: request.invocation_id.clone(),
                    reply: Some(invocation_reply::Reply::Terminal(TerminalOutcome {
                        invocation_id: request.invocation_id,
                        result: Some(terminal_outcome::Result::Succeeded(result)),
                    })),
                }))?;
            }
            Some(protocol_envelope::Body::Deactivate(_)) => return Ok(()),
            _ => {}
        }
    }
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !value.len().is_multiple_of(2) {
        return Err("one-time token has odd hex length".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
        .collect()
}
