// SPDX-License-Identifier: GPL-3.0-or-later
//! Ports between the Host-owned scheduler and runtime-specific transports.

use deckweave_automation::IngressReliability;
use deckweave_domain::{
    automation::{CommandEnvelope, DeckWeaveError, InvocationReply, OperationDescriptor},
    endpoint::{RuntimeEndpointRef, StableEndpointRef},
    ids::{ActivationEpoch, ExecutionId, GraphNodeId, InvocationId, JobId},
    schema::SchemaRef,
};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeEventKind {
    Signal {
        source: RuntimeEndpointRef,
        payload: Value,
        source_ticks: u64,
        reliability: IngressReliability,
    },
    Invocation(InvocationReply),
    TopologyFault {
        endpoint: StableEndpointRef,
    },
    GrantFault,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeEvent {
    pub activation_epoch: ActivationEpoch,
    pub kind: RuntimeEventKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeCommand {
    Invoke {
        request: Box<CommandEnvelope>,
        operation: OperationDescriptor,
    },
    Feedback {
        run_id: ExecutionId,
        graph_node_id: GraphNodeId,
        activation_epoch: ActivationEpoch,
        target: RuntimeEndpointRef,
        schema: SchemaRef,
        value: Value,
    },
    Cancel {
        activation_epoch: ActivationEpoch,
        invocation_id: InvocationId,
        job_id: Option<JobId>,
        target: RuntimeEndpointRef,
        reason: String,
    },
    CleanupRun {
        activation_epoch: ActivationEpoch,
        run_id: ExecutionId,
        reason: String,
    },
    CleanupAll {
        activation_epoch: ActivationEpoch,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeDispatchFailure {
    pub error: DeckWeaveError,
    /// True means the transport may already have exposed the effect to the
    /// provider, so automatic retry is forbidden.
    pub possibly_sent: bool,
}

pub trait RuntimeIntentDispatcher: Send + Sync {
    fn dispatch(&self, command: RuntimeCommand) -> Result<(), RuntimeDispatchFailure>;
}

pub trait RuntimeEventSink: Send + Sync {
    fn publish(&self, event: RuntimeEvent);
}
