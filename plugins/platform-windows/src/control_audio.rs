// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    PlatformFailure,
    audio::{AudioClient, AudioCommand, AudioCommandError, AudioState, AudioWorker},
    wire::WireSender,
};
use deckweave_protocol::v1::{
    EndpointPath, InvocationRequest, OpaquePayload, SchemaReference, Signal, endpoint_path,
    protocol_envelope,
};
use serde_json::Value;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const AUDIO_NODE: &str = "windows-control-node";
const AUDIO_PROVIDER: &str = "org.deckweave.platform.windows.control";
const AUDIO_SERVICE: &str = "windows-platform";
const AUDIO_ELEMENT: &str = "audio";
const AUDIO_CAPABILITY: &str = "endpoint-control";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AudioInvocation {
    pub command: AudioCommand,
    pub timeout: Duration,
}

pub(crate) struct ControlAudio {
    worker: Option<AudioWorker>,
    client: AudioClient,
    committed: Arc<AtomicBool>,
    pending: Arc<Mutex<Option<AudioState>>>,
    sequence: Arc<AtomicU64>,
    bridge: Option<JoinHandle<()>>,
}

impl ControlAudio {
    pub fn start(wire: WireSender, generation: u64) -> Self {
        let (worker, feed) = AudioWorker::spawn();
        let client = worker.client();
        let committed = Arc::new(AtomicBool::new(false));
        let bridge_committed = Arc::clone(&committed);
        let pending = Arc::new(Mutex::new(None));
        let bridge_pending = Arc::clone(&pending);
        let sequence = Arc::new(AtomicU64::new(0));
        let bridge_sequence = Arc::clone(&sequence);
        let bridge = thread::Builder::new()
            .name("deckweave-audio-state".into())
            .spawn(move || {
                while let Ok(state) = feed.recv() {
                    if !bridge_committed.load(Ordering::Acquire) {
                        if let Ok(mut pending) = bridge_pending.lock() {
                            *pending = Some(state);
                        }
                        continue;
                    }
                    if send_state(&wire, generation, &bridge_sequence, &state).is_err() {
                        return;
                    }
                }
            })
            .ok();
        Self {
            worker: Some(worker),
            client,
            committed,
            pending,
            sequence,
            bridge,
        }
    }

    pub fn activation_committed(&self, wire: &WireSender, generation: u64) -> Result<(), String> {
        self.committed.store(true, Ordering::Release);
        let pending = self
            .pending
            .lock()
            .map_err(|_| "audio state lock poisoned".to_string())?
            .take();
        if let Some(state) = pending {
            send_state(wire, generation, &self.sequence, &state)?;
        }
        self.client.refresh().map_err(|error| format!("{error:?}"))
    }

    pub fn apply(&self, invocation: AudioInvocation) -> Result<AudioState, AudioCommandError> {
        self.client.apply(invocation.command, invocation.timeout)
    }
}

impl Drop for ControlAudio {
    fn drop(&mut self) {
        self.worker.take();
        if let Some(bridge) = self.bridge.take() {
            let _ = bridge.join();
        }
    }
}

pub(crate) fn parse_invocation(
    request: &InvocationRequest,
) -> Result<Option<AudioInvocation>, PlatformFailure> {
    let Some(target) = request.target.as_ref() else {
        return Err(PlatformFailure::InvalidInput);
    };
    if !is_audio_target(target) {
        return Ok(None);
    }
    let (operation, command) = match target.port_id.as_str() {
        "set-volume" => (
            "set-volume",
            AudioCommand::SetVolume(number_input(
                request,
                "org.deckweave.windows.volume_scalar",
            )?),
        ),
        "change-volume" => (
            "change-volume",
            AudioCommand::ChangeVolume(number_input(
                request,
                "org.deckweave.windows.volume_delta",
            )?),
        ),
        "set-mute" => (
            "set-mute",
            AudioCommand::SetMute(bool_input(request, "org.deckweave.windows.mute")?),
        ),
        "toggle-mute" => {
            unit_input(request, "org.deckweave.windows.unit")?;
            ("toggle-mute", AudioCommand::ToggleMute)
        }
        _ => return Ok(None),
    };
    if request.operation_id != operation {
        return Err(PlatformFailure::InvalidInput);
    }
    let budget = request
        .remaining_budget_ms
        .ok_or(PlatformFailure::DeadlineExceeded)?;
    if budget == 0 {
        return Err(PlatformFailure::DeadlineExceeded);
    }
    Ok(Some(AudioInvocation {
        command,
        timeout: Duration::from_millis(budget.min(5_000)),
    }))
}

pub(crate) fn state_payload(state: &AudioState) -> Result<OpaquePayload, PlatformFailure> {
    Ok(OpaquePayload {
        schema: Some(SchemaReference {
            id: "org.deckweave.windows.audio_state".into(),
            major: 1,
            minor: 0,
        }),
        value: serde_json::to_vec(state).map_err(|_| PlatformFailure::OsApiUnavailable)?,
    })
}

fn is_audio_target(target: &EndpointPath) -> bool {
    target.node_id == AUDIO_NODE
        && target.provider_id == AUDIO_PROVIDER
        && target.element_id == AUDIO_ELEMENT
        && target.capability_id == AUDIO_CAPABILITY
        && matches!(
            target.target.as_ref(),
            Some(endpoint_path::Target::ServiceId(service)) if service == AUDIO_SERVICE
        )
}

fn input_value(
    request: &InvocationRequest,
    expected_schema: &str,
) -> Result<Value, PlatformFailure> {
    let input = request
        .input
        .as_ref()
        .ok_or(PlatformFailure::InvalidInput)?;
    if input.value.len() > 64 {
        return Err(PlatformFailure::InvalidInput);
    }
    let schema = input.schema.as_ref().ok_or(PlatformFailure::InvalidInput)?;
    if schema.id != expected_schema || schema.major != 1 || schema.minor != 0 {
        return Err(PlatformFailure::InvalidInput);
    }
    serde_json::from_slice(&input.value).map_err(|_| PlatformFailure::InvalidInput)
}

fn number_input(request: &InvocationRequest, schema: &str) -> Result<f64, PlatformFailure> {
    input_value(request, schema)?
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or(PlatformFailure::InvalidInput)
}

fn bool_input(request: &InvocationRequest, schema: &str) -> Result<bool, PlatformFailure> {
    input_value(request, schema)?
        .as_bool()
        .ok_or(PlatformFailure::InvalidInput)
}

fn unit_input(request: &InvocationRequest, schema: &str) -> Result<(), PlatformFailure> {
    if !input_value(request, schema)?.is_null() {
        return Err(PlatformFailure::InvalidInput);
    }
    Ok(())
}

#[allow(deprecated)]
fn send_state(
    wire: &WireSender,
    generation: u64,
    sequence: &AtomicU64,
    state: &AudioState,
) -> Result<(), String> {
    let sequence = sequence.fetch_add(1, Ordering::AcqRel).saturating_add(1);
    let payload = state_payload(state).map_err(|error| format!("{error:?}"))?;
    wire.send(protocol_envelope::Body::Signal(Signal {
        source_port_id: String::new(),
        device_session_generation: generation,
        sequence,
        captured_ticks: sequence,
        clock_domain_id: "org.deckweave.platform.windows.monotonic".into(),
        wall_clock_rfc3339: None,
        payload: Some(payload),
        source: Some(EndpointPath {
            node_id: AUDIO_NODE.into(),
            provider_id: AUDIO_PROVIDER.into(),
            target: Some(endpoint_path::Target::ServiceId(AUDIO_SERVICE.into())),
            element_id: AUDIO_ELEMENT.into(),
            capability_id: AUDIO_CAPABILITY.into(),
            port_id: "state".into(),
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        operation: &str,
        port: &str,
        schema: &str,
        value: Value,
    ) -> Result<InvocationRequest, Box<dyn std::error::Error>> {
        Ok(InvocationRequest {
            configured_instance_id: None,
            invocation_id: "invocation-1".into(),
            run_id: "run-1".into(),
            operation_id: operation.into(),
            activation_epoch: 1,
            descriptor_revision: 1,
            deadline_host_ticks: 100,
            idempotency_key: None,
            input: Some(OpaquePayload {
                schema: Some(SchemaReference {
                    id: schema.into(),
                    major: 1,
                    minor: 0,
                }),
                value: serde_json::to_vec(&value)?,
            }),
            target: Some(EndpointPath {
                node_id: AUDIO_NODE.into(),
                provider_id: AUDIO_PROVIDER.into(),
                target: Some(endpoint_path::Target::ServiceId(AUDIO_SERVICE.into())),
                element_id: AUDIO_ELEMENT.into(),
                capability_id: AUDIO_CAPABILITY.into(),
                port_id: port.into(),
            }),
            remaining_budget_ms: Some(500),
        })
    }

    #[test]
    fn full_endpoint_schema_and_operation_select_one_audio_command()
    -> Result<(), Box<dyn std::error::Error>> {
        let invocation = parse_invocation(&request(
            "change-volume",
            "change-volume",
            "org.deckweave.windows.volume_delta",
            serde_json::json!(-0.1),
        )?)?
        .ok_or("audio invocation")?;
        assert_eq!(invocation.command, AudioCommand::ChangeVolume(-0.1));
        Ok(())
    }

    #[test]
    fn mismatched_operation_or_schema_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            parse_invocation(&request(
                "set-mute",
                "set-volume",
                "org.deckweave.windows.volume_scalar",
                serde_json::json!(0.5),
            )?),
            Err(PlatformFailure::InvalidInput)
        );
        assert_eq!(
            parse_invocation(&request(
                "set-volume",
                "set-volume",
                "org.deckweave.windows.mute",
                serde_json::json!(true),
            )?),
            Err(PlatformFailure::InvalidInput)
        );
        Ok(())
    }

    #[test]
    fn state_payload_uses_the_declared_wire_schema() -> Result<(), Box<dyn std::error::Error>> {
        let payload = state_payload(&AudioState {
            endpoint_ref: "endpoint-a".into(),
            scalar: 0.5,
            muted: false,
            availability: crate::audio::AudioAvailability::Available,
            revision: 2,
        })?;
        assert_eq!(
            payload.schema.ok_or("schema")?.id,
            "org.deckweave.windows.audio_state"
        );
        let value: Value = serde_json::from_slice(&payload.value)?;
        assert_eq!(value["endpointRef"], "endpoint-a");
        assert_eq!(value["revision"], 2);
        Ok(())
    }
}
