// SPDX-License-Identifier: GPL-3.0-or-later
use crate::PlatformFailure;
use deckweave_protocol::{
    FEATURE_ACTIVATION_COMMIT, FEATURE_INVOCATION_ENDPOINT_PATH,
    framing::{read_envelope, write_envelope},
    v1::{
        ActivationReply, FeatureRequirement, Heartbeat, Hello, InvocationReply, OpaquePayload,
        ProtocolEnvelope, ProtocolLimits, ProtocolVersion, ProtocolVersionRange, SchemaReference,
        invocation_reply, protocol_envelope,
    },
};
use std::{error::Error, io};

const CONTROL_DESCRIPTOR: &[u8] = include_bytes!("../assets/control-descriptors.json");
const INPUT_DESCRIPTOR: &[u8] = include_bytes!("../assets/input-observe-descriptors.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGroup {
    Control,
    InputObserve,
}

impl RuntimeGroup {
    fn id(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::InputObserve => "input-observe",
        }
    }

    fn contribution(self) -> &'static str {
        match self {
            Self::Control => "windows-control",
            Self::InputObserve => "keyboard-observe",
        }
    }

    fn descriptor(self) -> &'static [u8] {
        match self {
            Self::Control => CONTROL_DESCRIPTOR,
            Self::InputObserve => INPUT_DESCRIPTOR,
        }
    }
}

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

pub fn run(expected_group: RuntimeGroup) -> Result<(), Box<dyn Error>> {
    let package_id = std::env::var("DECKWEAVE_PACKAGE_ID")?;
    if package_id != "org.deckweave.platform.windows" {
        return Err("unexpected package identity".into());
    }
    let runtime_group = std::env::var("DECKWEAVE_RUNTIME_GROUP")?;
    if runtime_group != expected_group.id() {
        return Err("runtime artifact started for the wrong group".into());
    }
    let token = decode_hex(&std::env::var("DECKWEAVE_ONE_TIME_TOKEN")?)?;
    let generation = std::env::var("DECKWEAVE_SESSION_GENERATION")?.parse()?;
    let maximum_message_bytes = std::env::var("DECKWEAVE_MAXIMUM_MESSAGE_BYTES")?.parse()?;
    let mut runtime = Runtime {
        generation,
        maximum_message_bytes,
        next_message_id: 0,
    };
    let mut features = vec![FeatureRequirement {
        id: FEATURE_ACTIVATION_COMMIT.into(),
        minimum_version: 1,
        maximum_version: 1,
        required: true,
    }];
    if expected_group == RuntimeGroup::Control {
        features.push(FeatureRequirement {
            id: FEATURE_INVOCATION_ENDPOINT_PATH.into(),
            minimum_version: 1,
            maximum_version: 1,
            required: true,
        });
    }
    runtime.send(protocol_envelope::Body::Hello(Hello {
        protocol: Some(ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        }),
        package_id,
        runtime_group,
        one_time_token: token,
        features,
        schemas: vec![],
        limits: Some(ProtocolLimits {
            maximum_message_bytes: u64::try_from(maximum_message_bytes)?,
            maximum_chunk_bytes: 64 * 1024,
            maximum_concurrent_jobs: 8,
            maximum_artifact_bytes: 1024 * 1024,
            maximum_events_per_second: 512,
        }),
    }))?;

    serve(&mut runtime, expected_group)
}

fn serve(runtime: &mut Runtime, expected_group: RuntimeGroup) -> Result<(), Box<dyn Error>> {
    let mut stdin = io::stdin().lock();
    while let Some(envelope) = read_envelope(&mut stdin, runtime.maximum_message_bytes)? {
        match envelope.body {
            Some(protocol_envelope::Body::HelloAck(ack)) => {
                let maximum = ack
                    .limits
                    .ok_or("HelloAck has no limits")?
                    .maximum_message_bytes;
                runtime.maximum_message_bytes = usize::try_from(maximum)?;
            }
            Some(protocol_envelope::Body::Activate(request)) => {
                if request.contribution_id != expected_group.contribution() {
                    return Err("contribution does not belong to this runtime group".into());
                }
                runtime.send(protocol_envelope::Body::Activated(ActivationReply {
                    runtime_lease_id: request.runtime_lease_id,
                    descriptor_revision: 1,
                    descriptor: Some(OpaquePayload {
                        schema: Some(SchemaReference {
                            id: "org.deckweave.descriptor.node_list".into(),
                            major: 1,
                            minor: 0,
                        }),
                        value: expected_group.descriptor().to_vec(),
                    }),
                }))?;
                runtime.send(protocol_envelope::Body::Heartbeat(Heartbeat {
                    host_ticks: 0,
                    active_jobs: 0,
                }))?;
            }
            Some(protocol_envelope::Body::Invoke(request)) => {
                runtime.send(protocol_envelope::Body::InvocationReply(InvocationReply {
                    invocation_id: request.invocation_id,
                    reply: Some(invocation_reply::Reply::Rejected(
                        PlatformFailure::controlled_adapter().structured(),
                    )),
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
