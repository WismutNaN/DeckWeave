// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    PlatformFailure, RuntimeGroup, control_audio::ControlAudio, control_window::ControlWindow,
    descriptor::runtime_descriptor, invocation::invoke, wire::WireSender,
};
use deckweave_protocol::{
    FEATURE_ACTIVATION_COMMIT, FEATURE_INVOCATION_ENDPOINT_PATH,
    framing::read_envelope,
    v1::{
        ActivationReply, FeatureRequirement, Heartbeat, Hello, InvocationReply, OpaquePayload,
        ProtocolLimits, ProtocolVersion, ProtocolVersionRange, SchemaReference, invocation_reply,
        protocol_envelope,
    },
};
use std::{error::Error, io};

pub fn run(expected_group: RuntimeGroup) -> Result<(), Box<dyn Error>> {
    let package_id = std::env::var("DECKWEAVE_PACKAGE_ID")?;
    if package_id != expected_group.package() {
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
    let audio = expected_group
        .audio()
        .then(|| ControlAudio::start(wire.clone(), generation));
    let window = expected_group
        .windows()
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
    if group.broker() {
        features.push(FeatureRequirement {
            id: deckweave_protocol::FEATURE_HOST_PROCESS_BROKER.into(),
            minimum_version: 1,
            maximum_version: 1,
            required: false,
        });
    }
    if !group.input() {
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
    let (incoming, receiver) = std::sync::mpsc::sync_channel(16);
    let reader_limit = wire.read_limit();
    // The process-owned reader may block on the host pipe; the runtime loop remains
    // able to stop a failed adapter without waiting for another host command.
    std::thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        loop {
            let next = read_envelope(
                &mut stdin,
                reader_limit.load(std::sync::atomic::Ordering::Acquire),
            )
            .map_err(|error| error.to_string());
            let terminal = !matches!(&next, Ok(Some(_)));
            if incoming.send(next).is_err() || terminal {
                return;
            }
        }
    });
    let mut acknowledged = false;
    let mut activated = false;
    let mut epoch = None;
    let mut last_message = 0;
    let mut heartbeat = None;
    let mut input = None;
    let mut broker = crate::process_broker::Broker::default();
    loop {
        broker.expire(wire)?;
        if wire.failed() {
            return Err("runtime adapter or transport fault".into());
        }
        let envelope = match receiver.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(Ok(Some(envelope))) => envelope,
            Ok(Ok(None)) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Err(error)) => return Err(error.into()),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
        };
        if envelope.session_generation != generation || envelope.message_id <= last_message {
            return Err("stale or replayed host message".into());
        }
        last_message = envelope.message_id;
        match envelope.body {
            Some(protocol_envelope::Body::HelloAck(ack)) => {
                broker.enabled = ack.features.iter().any(|f| {
                    f.id == deckweave_protocol::FEATURE_HOST_PROCESS_BROKER && f.version == 1
                });
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
                        value: runtime_descriptor(expected_group, broker.enabled)?,
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
                if expected_group.input() && cfg!(windows) {
                    input = Some(crate::input::InputWorker::start(wire.clone(), generation)?);
                }
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
                    || !request
                        .target
                        .as_ref()
                        .is_some_and(|p| expected_group.allows(&p.element_id, &p.capability_id))
                {
                    wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
                        invocation_id: request.invocation_id,
                        reply: Some(invocation_reply::Reply::Rejected(
                            PlatformFailure::InvalidInput.structured(),
                        )),
                    }))?;
                    continue;
                }
                if !(expected_group.broker() && broker.invoke(wire, &request)?) {
                    invoke(wire, audio.as_ref(), window.as_ref(), request)?;
                }
            }
            Some(protocol_envelope::Body::HostProcessReply(reply)) => broker.reply(wire, reply)?,
            Some(protocol_envelope::Body::Deactivate(_)) => return Ok(()),
            _ => {}
        }
    }
    drop(heartbeat);
    drop(input);
    Ok(())
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
