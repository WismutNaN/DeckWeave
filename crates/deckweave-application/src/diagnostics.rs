// SPDX-License-Identifier: GPL-3.0-or-later
//! Transport-independent runtime diagnostics projection.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTraceEventKind {
    Activation,
    IngressSignal,
    IngressInvocation,
    IngressFault,
    QueueCoalesced,
    IntentInvoke,
    IntentFeedback,
    IntentCancel,
    Cleanup,
    DispatchRejected,
    Barrier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTraceEntry {
    pub sequence: u64,
    pub activation_epoch: Option<u64>,
    pub kind: RuntimeTraceEventKind,
    pub run_id: Option<String>,
    pub invocation_id: Option<String>,
    pub graph_node_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    /// Machine status only. Payloads, private text and credentials are never
    /// included in the default trace projection.
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTraceSnapshot {
    pub format_version: u32,
    pub plan_hash: Option<String>,
    pub schema_hashes: Vec<String>,
    pub entries: Vec<RuntimeTraceEntry>,
    pub dropped_entries: u64,
    pub capacity: usize,
}

pub trait RuntimeDiagnosticsSource: Send + Sync {
    fn runtime_trace(&self) -> RuntimeTraceSnapshot;
}

#[derive(Default)]
pub(crate) struct EmptyRuntimeDiagnostics;

impl RuntimeDiagnosticsSource for EmptyRuntimeDiagnostics {
    fn runtime_trace(&self) -> RuntimeTraceSnapshot {
        RuntimeTraceSnapshot {
            format_version: 1,
            plan_hash: None,
            schema_hashes: vec![],
            entries: vec![],
            dropped_entries: 0,
            capacity: 0,
        }
    }
}
