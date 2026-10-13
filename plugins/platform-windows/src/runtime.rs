// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    PlatformFailure,
    audio::EffectKnowledge,
    control_audio::{ControlAudio, parse_invocation, state_payload},
    control_window::{
        ControlWindow, WindowResult, parse_invocation as parse_window_invocation,
        result_payload as window_result_payload,
    },
    descriptor::runtime_descriptor,
    wire::WireSender,
};
use deckweave_protocol::{
    FEATURE_ACTIVATION_COMMIT, FEATURE_INVOCATION_ENDPOINT_PATH,
    framing::read_envelope,
    v1::{
        ActivationReply, FeatureRequirement, Heartbeat, Hello, InvocationReply, OpaquePayload,
        ProtocolLimits, ProtocolVersion, ProtocolVersionRange, SchemaReference, TerminalOutcome,
        invocation_reply, protocol_envelope, terminal_outcome,
    },
};
use std::{error::Error, io};

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
    let (wire, writer) = WireSender::start(generation, maximum_message_bytes)?;
    wire.send(protocol_envelope::Body::Hello(hello(
        expected_group,
        package_id,
        token,
        maximum_message_bytes,
    )))?;
    let audio = (expected_group == RuntimeGroup::Control)
        .then(|| ControlAudio::start(wire.clone(), generation));
    let window = (expected_group == RuntimeGroup::Control)
        .then(|| ControlWindow::start(wire.clone(), generation));
    let result = serve(expected_group, generation, &wire, audio, window);
    drop(wire);
    let writer_result = writer
        .join()
        .map_err(|_| "wire writer panicked")?
        .map_err(|error| -> Box<dyn Error> { error.into() });
    result?;
    writer_result
}

fn hello(
    group: RuntimeGroup,
    package_id: String,
    token: Vec<u8>,
    maximum_message_bytes: usize,
) -> Hello {
    let mut features = vec![FeatureRequirement {
        id: FEATURE_ACTIVATION_COMMIT.into(),
        minimum_version: 1,
        maximum_version: 1,
        required: true,
    }];
    if group == RuntimeGroup::Control {
        features.push(FeatureRequirement {
            id: FEATURE_INVOCATION_ENDPOINT_PATH.into(),
            minimum_version: 1,
            maximum_version: 1,
            required: true,
        });
    }
    Hello {
        protocol: Some(ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        }),
        package_id,
        runtime_group: group.id().into(),
        one_time_token: token,
        features,
        schemas: vec![],
        limits: Some(ProtocolLimits {
            maximum_message_bytes: u64::try_from(maximum_message_bytes).unwrap_or(u64::MAX),
            maximum_chunk_bytes: 64 * 1024,
            maximum_concurrent_jobs: 8,
            maximum_artifact_bytes: 1024 * 1024,
            maximum_events_per_second: 512,
        }),
    }
}

fn serve(
    expected_group: RuntimeGroup,
    generation: u64,
    wire: &WireSender,
    audio: Option<ControlAudio>,
    window: Option<ControlWindow>,
) -> Result<(), Box<dyn Error>> {
    let mut stdin = io::stdin().lock();
    let mut acknowledged = false;
    let mut activated = false;
    let mut epoch = None;
    let mut last_message = 0;
    let mut heartbeat = None;
    while let Some(envelope) = read_envelope(&mut stdin, wire.maximum_message_bytes())? {
        if envelope.session_generation != generation || envelope.message_id <= last_message {
            return Err("stale or replayed host message".into());
        }
        last_message = envelope.message_id;
        match envelope.body {
            Some(protocol_envelope::Body::HelloAck(ack)) => {
                if acknowledged {
                    return Err("duplicate HelloAck".into());
                }
                let maximum = ack
                    .limits
                    .ok_or("HelloAck has no limits")?
                    .maximum_message_bytes;
                let maximum = usize::try_from(maximum)?;
                if maximum == 0 || maximum > wire.maximum_message_bytes() {
                    return Err("invalid negotiated limit".into());
                }
                wire.set_maximum_message_bytes(maximum);
                acknowledged = true;
            }
            Some(protocol_envelope::Body::Activate(request)) => {
                if !acknowledged
                    || activated
                    || request.configured_instance_id.is_some()
                    || request.contribution_id != expected_group.contribution()
                {
                    return Err("contribution does not belong to this runtime group".into());
                }
                wire.send(protocol_envelope::Body::Activated(ActivationReply {
                    runtime_lease_id: request.runtime_lease_id,
                    descriptor_revision: 1,
                    descriptor: Some(OpaquePayload {
                        schema: Some(SchemaReference {
                            id: "org.deckweave.descriptor.node_list".into(),
                            major: 1,
                            minor: 0,
                        }),
                        value: runtime_descriptor(expected_group)?,
                    }),
                }))?;
                wire.send(protocol_envelope::Body::Heartbeat(Heartbeat {
                    host_ticks: 0,
                    active_jobs: 0,
                }))?;
                activated = true;
                heartbeat = Some(crate::heartbeat::HeartbeatWorker::start(wire.clone())?);
            }
            Some(protocol_envelope::Body::ActivationCommitted(commit)) => {
                if !activated || epoch.is_some() || commit.activation_epoch == 0 {
                    return Err("invalid activation commit".into());
                }
                epoch = Some(commit.activation_epoch);
                if let Some(audio) = &audio {
                    audio.activation_committed(wire, generation)?;
                }
                if let Some(window) = &window {
                    window.activation_committed(wire, generation)?;
                }
            }
            Some(protocol_envelope::Body::Invoke(request)) => {
                if epoch != Some(request.activation_epoch)
                    || request.descriptor_revision != 1
                    || request.configured_instance_id.is_some()
                {
                    wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
                        invocation_id: request.invocation_id,
                        reply: Some(invocation_reply::Reply::Rejected(
                            PlatformFailure::InvalidInput.structured(),
                        )),
                    }))?;
                    continue;
                }
                invoke(wire, audio.as_ref(), window.as_ref(), request)?;
            }
            Some(protocol_envelope::Body::Deactivate(_)) => return Ok(()),
            _ => {}
        }
    }
    drop(heartbeat);
    Ok(())
}

fn invoke(
    wire: &WireSender,
    audio: Option<&ControlAudio>,
    window: Option<&ControlWindow>,
    request: deckweave_protocol::v1::InvocationRequest,
) -> Result<(), Box<dyn Error>> {
    let invocation_id = request.invocation_id.clone();
    let reply = match parse_invocation(&request) {
        Ok(Some(invocation)) => match audio
            .ok_or(PlatformFailure::OsApiUnavailable)?
            .apply(invocation)
        {
            Ok(state) => invocation_reply::Reply::Terminal(TerminalOutcome {
                invocation_id: invocation_id.clone(),
                result: Some(terminal_outcome::Result::Succeeded(state_payload(&state)?)),
            }),
            Err(error) if error.effect == EffectKnowledge::PossiblyApplied => {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id: invocation_id.clone(),
                    result: Some(terminal_outcome::Result::UnknownOutcome(
                        PlatformFailure::from(error.failure).structured(),
                    )),
                })
            }
            Err(error) => {
                invocation_reply::Reply::Rejected(PlatformFailure::from(error.failure).structured())
            }
        },
        Ok(None) => window_reply(window, &request)?,
        Err(error) => invocation_reply::Reply::Rejected(error.structured()),
    };
    wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
        invocation_id,
        reply: Some(reply),
    }))?;
    Ok(())
}

fn window_reply(
    window: Option<&ControlWindow>,
    request: &deckweave_protocol::v1::InvocationRequest,
) -> Result<invocation_reply::Reply, Box<dyn Error>> {
    let invocation_id = request.invocation_id.clone();
    let reply = match parse_window_invocation(request) {
        Ok(Some(invocation)) => match window
            .ok_or(PlatformFailure::OsApiUnavailable)?
            .apply(invocation)
        {
            Ok(result @ (WindowResult::State(_) | WindowResult::Catalog(_))) => {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id,
                    result: Some(terminal_outcome::Result::Succeeded(window_result_payload(
                        &result,
                    )?)),
                })
            }
            Err(error)
                if matches!(
                    error.failure,
                    crate::window::WindowFailure::ForegroundDenied
                        | crate::window::WindowFailure::StaleReference
                ) =>
            {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id,
                    result: Some(terminal_outcome::Result::Failed(
                        PlatformFailure::from(error.failure).structured(),
                    )),
                })
            }
            Err(error) if error.effect == crate::window::EffectKnowledge::PossiblyApplied => {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id,
                    result: Some(terminal_outcome::Result::UnknownOutcome(
                        PlatformFailure::from(error.failure).structured(),
                    )),
                })
            }
            Err(error) => {
                invocation_reply::Reply::Rejected(PlatformFailure::from(error.failure).structured())
            }
        },
        Ok(None) => {
            invocation_reply::Reply::Rejected(PlatformFailure::controlled_adapter().structured())
        }
        Err(error) => invocation_reply::Reply::Rejected(error.structured()),
    };
    Ok(reply)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !value.is_ascii() || !value.len().is_multiple_of(2) {
        return Err("one-time token has odd hex length".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
        .collect()
}
