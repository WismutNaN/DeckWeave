// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    window::{
        ActiveWindowState, WindowCatalog, WindowCatalogRequest, WindowClient, WindowCommandError,
        WindowFailure, WindowReference, WindowSelector, WindowWorker,
    },
    wire::WireSender,
};
use deckweave_protocol::v1::{EndpointPath, Signal, endpoint_path, protocol_envelope};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const NODE: &str = "windows-control-node";
const PROVIDER: &str = "org.deckweave.platform.windows.control";
const SERVICE: &str = "windows-platform";
const ELEMENT: &str = "applications";

mod contract;
use contract::payload;
pub(crate) use contract::{parse_invocation, result_payload};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WindowInvocation {
    Activate(WindowReference, Duration),
    Focus(WindowSelector, Duration),
    Catalog(WindowCatalogRequest, Duration),
}

pub(crate) enum WindowResult {
    State(ActiveWindowState),
    Catalog(WindowCatalog),
}

pub(crate) struct ControlWindow {
    worker: Option<WindowWorker>,
    client: WindowClient,
    committed: Arc<AtomicBool>,
    pending: Arc<Mutex<Option<ActiveWindowState>>>,
    sequence: Arc<AtomicU64>,
    bridge: Option<JoinHandle<()>>,
}

impl ControlWindow {
    pub(crate) fn start(wire: WireSender, generation: u64) -> Self {
        let (worker, feed) = WindowWorker::spawn(generation);
        let client = worker.client();
        let committed = Arc::new(AtomicBool::new(false));
        let bridge_committed = Arc::clone(&committed);
        let pending = Arc::new(Mutex::new(None));
        let bridge_pending = Arc::clone(&pending);
        let sequence = Arc::new(AtomicU64::new(0));
        let bridge_sequence = Arc::clone(&sequence);
        let bridge = thread::Builder::new()
            .name("deckweave-window-state".into())
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

    pub(crate) fn activation_committed(
        &self,
        wire: &WireSender,
        generation: u64,
    ) -> Result<(), String> {
        self.committed.store(true, Ordering::Release);
        let pending = self
            .pending
            .lock()
            .map_err(|_| "window state lock poisoned".to_string())?
            .take();
        if let Some(state) = pending {
            send_state(wire, generation, &self.sequence, &state)?;
        }
        self.client.refresh().map_err(|error| format!("{error:?}"))
    }

    pub(crate) fn apply(
        &self,
        invocation: WindowInvocation,
    ) -> Result<WindowResult, WindowCommandError> {
        match invocation {
            WindowInvocation::Activate(target, timeout) => self
                .client
                .activate(target, timeout)
                .map(WindowResult::State),
            WindowInvocation::Focus(selector, timeout) => self
                .client
                .focus(selector, timeout)
                .map(WindowResult::State),
            WindowInvocation::Catalog(request, timeout) => {
                let catalog = self.client.catalog(timeout)?;
                page_catalog(catalog, &request).map(WindowResult::Catalog)
            }
        }
    }
}

impl Drop for ControlWindow {
    fn drop(&mut self) {
        self.worker.take();
        if let Some(bridge) = self.bridge.take() {
            let _ = bridge.join();
        }
    }
}

fn page_catalog(
    mut catalog: WindowCatalog,
    request: &WindowCatalogRequest,
) -> Result<WindowCatalog, WindowCommandError> {
    let offset = usize::try_from(request.offset)
        .map_err(|_| WindowCommandError::before_effect(WindowFailure::InvalidInput))?;
    if offset > catalog.windows.len() {
        return Err(WindowCommandError::before_effect(
            WindowFailure::InvalidInput,
        ));
    }
    let end = offset
        .saturating_add(request.limit as usize)
        .min(catalog.windows.len());
    let page = catalog.windows[offset..end].to_vec();
    catalog.next_offset =
        (end < catalog.windows.len()).then(|| u32::try_from(end).unwrap_or(u32::MAX));
    catalog.windows = page;
    Ok(catalog)
}

#[allow(deprecated)]
fn send_state(
    wire: &WireSender,
    generation: u64,
    sequence: &AtomicU64,
    state: &ActiveWindowState,
) -> Result<(), String> {
    let sequence = sequence.fetch_add(1, Ordering::AcqRel).saturating_add(1);
    wire.send(protocol_envelope::Body::Signal(Signal {
        source_port_id: String::new(),
        device_session_generation: generation,
        sequence,
        captured_ticks: sequence,
        clock_domain_id: "org.deckweave.platform.windows.monotonic".into(),
        wall_clock_rfc3339: None,
        payload: Some(
            payload("org.deckweave.windows.active_window_state", state)
                .map_err(|error| format!("{error:?}"))?,
        ),
        source: Some(EndpointPath {
            node_id: NODE.into(),
            provider_id: PROVIDER.into(),
            target: Some(endpoint_path::Target::ServiceId(SERVICE.into())),
            element_id: ELEMENT.into(),
            capability_id: "foreground-state".into(),
            port_id: "state".into(),
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlatformFailure;
    use deckweave_protocol::v1::{InvocationRequest, OpaquePayload, SchemaReference};

    fn request(
        operation: &str,
        capability: &str,
        port: &str,
        schema: &str,
        value: serde_json::Value,
    ) -> Result<InvocationRequest, Box<dyn std::error::Error>> {
        Ok(InvocationRequest {
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
                node_id: NODE.into(),
                provider_id: PROVIDER.into(),
                target: Some(endpoint_path::Target::ServiceId(SERVICE.into())),
                element_id: ELEMENT.into(),
                capability_id: capability.into(),
                port_id: port.into(),
            }),
            remaining_budget_ms: Some(500),
        })
    }

    #[test]
    fn full_path_schema_and_operation_are_required() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_invocation(&request(
            "focus-application",
            "window-activation",
            "focus",
            "org.deckweave.windows.window_selector",
            serde_json::json!({"executable":"C:\\App\\app.exe","titleContains":null}),
        )?)?
        .ok_or("window invocation")?;
        assert!(matches!(parsed, WindowInvocation::Focus(_, _)));
        assert_eq!(
            parse_invocation(&request(
                "activate-window",
                "window-activation",
                "focus",
                "org.deckweave.windows.window_selector",
                serde_json::json!({"executable":"C:\\App\\app.exe"}),
            )?),
            Err(PlatformFailure::InvalidInput)
        );
        Ok(())
    }

    #[test]
    fn catalog_result_uses_declared_schema() -> Result<(), Box<dyn std::error::Error>> {
        let result = WindowResult::Catalog(WindowCatalog {
            generation: 4,
            windows: Vec::new(),
            total: 0,
            next_offset: None,
            revision: 1,
        });
        assert_eq!(
            result_payload(&result)?.schema.ok_or("schema")?.id,
            "org.deckweave.windows.window_catalog"
        );
        Ok(())
    }

    #[test]
    fn catalog_pages_stay_bounded_and_keep_total() -> Result<(), Box<dyn std::error::Error>> {
        let windows = (0..12)
            .map(|index| crate::window::WindowCatalogItem {
                window_ref: format!("{index:064x}"),
                process_id: index + 1,
                executable: Some(format!("C:\\App\\{index}.exe")),
                title: if index == 0 {
                    String::new()
                } else {
                    format!("Window {index}")
                },
            })
            .collect();
        let catalog = WindowCatalog {
            generation: 4,
            windows,
            total: 12,
            next_offset: None,
            revision: 1,
        };
        let Ok(first) = page_catalog(
            catalog.clone(),
            &WindowCatalogRequest {
                offset: 0,
                limit: 10,
            },
        ) else {
            return Err("first page failed".into());
        };
        assert_eq!(first.windows.len(), 10);
        assert_eq!(first.total, 12);
        assert_eq!(first.next_offset, Some(10));
        assert!(first.windows[0].title.is_empty());
        let Ok(last) = page_catalog(
            catalog,
            &WindowCatalogRequest {
                offset: 10,
                limit: 10,
            },
        ) else {
            return Err("last page failed".into());
        };
        assert_eq!(last.windows.len(), 2);
        assert_eq!(last.next_offset, None);
        Ok(())
    }
}
