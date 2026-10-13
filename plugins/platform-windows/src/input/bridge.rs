// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    native::{Record, Source},
    normalizer::Normalizer,
};
use crate::wire::WireSender;
use deckweave_protocol::v1::{
    EndpointPath, OpaquePayload, SchemaReference, Signal, endpoint_path, protocol_envelope,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) struct InputWorker {
    source: Option<Source>,
    bridge: Option<JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
}

impl InputWorker {
    pub fn start(wire: WireSender, generation: u64) -> Result<Self, String> {
        let (source, feed, keys, quarantined, failed) = Source::start()?;
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let bridge = thread::Builder::new()
            .name("windows-keyboard-normalizer".into())
            .spawn(move || {
                let mut normalizer = Normalizer::new(keys, quarantined);
                let mut output = Output {
                    wire,
                    generation,
                    sequence: 0,
                    clock: Instant::now(),
                };
                loop {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    if failed.load(Ordering::Acquire) {
                        output.wire.fail();
                        break;
                    }
                    let record = match feed.recv_timeout(Duration::from_millis(50)) {
                        Ok(record) => record,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => {
                            if !stop.load(Ordering::Acquire) {
                                output.wire.fail();
                            }
                            break;
                        }
                    };
                    let result = match record {
                        Record::Removed(device) => {
                            normalizer.remove(device).into_iter().try_for_each(|key| {
                                output.send(&key, "button", "pressed", "boolean", &false)
                            })
                        }
                        Record::Key {
                            device,
                            scan,
                            flags,
                            vkey,
                            neutral,
                        } => match normalizer.key(device, scan, flags, vkey, neutral) {
                            Ok(Some(event)) => output
                                .send(
                                    "keyboard",
                                    "observe",
                                    "edge",
                                    "keyboard_observation",
                                    &event.observation,
                                )
                                .and_then(|()| {
                                    if let Some(pressed) = event.pressed {
                                        output.send(
                                            &event.key, "button", "pressed", "boolean", &pressed,
                                        )
                                    } else {
                                        Ok(())
                                    }
                                }),
                            Ok(None) => Ok(()),
                            Err(error) => Err(error.into()),
                        },
                    };
                    if result.is_err() {
                        output.wire.fail();
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            source: Some(source),
            bridge: Some(bridge),
            stopping,
        })
    }
}

impl Drop for InputWorker {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.source.take();
        if let Some(bridge) = self.bridge.take() {
            let _ = bridge.join();
        }
    }
}

struct Output {
    wire: WireSender,
    generation: u64,
    sequence: u64,
    clock: Instant,
}
impl Output {
    #[allow(deprecated)]
    fn send(
        &mut self,
        element: &str,
        capability: &str,
        port: &str,
        schema: &str,
        value: &impl serde::Serialize,
    ) -> Result<(), String> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("input sequence exhausted")?;
        self.wire.send(protocol_envelope::Body::Signal(Signal {
            source_port_id: String::new(),
            device_session_generation: self.generation,
            sequence: self.sequence,
            captured_ticks: u64::try_from(self.clock.elapsed().as_micros()).unwrap_or(u64::MAX),
            clock_domain_id: "org.deckweave.windows.input.monotonic_microseconds".into(),
            wall_clock_rfc3339: None,
            payload: Some(OpaquePayload {
                schema: Some(SchemaReference {
                    id: format!("org.deckweave.windows.{schema}"),
                    major: 1,
                    minor: 0,
                }),
                value: serde_json::to_vec(value).map_err(|e| e.to_string())?,
            }),
            source: Some(EndpointPath {
                node_id: "windows-input-node".into(),
                provider_id: "org.deckweave.platform.windows.input".into(),
                target: Some(endpoint_path::Target::DeviceIdentity(
                    "system-keyboard-aggregate".into(),
                )),
                element_id: element.into(),
                capability_id: capability.into(),
                port_id: port.into(),
            }),
        }))
    }
}
