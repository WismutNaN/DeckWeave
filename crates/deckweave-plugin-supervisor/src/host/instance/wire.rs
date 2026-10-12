// SPDX-License-Identifier: GPL-3.0-or-later
//! Translation between Host dispatch vocabulary and the permissive wire protocol.

use crate::{host::HostRuntimeError, routing::normalize_signal_source};
use deckweave_automation::IngressReliability;
use deckweave_domain::{
    automation::{CommandOutcome, DeckWeaveError, ErrorCategory, InvocationReply},
    endpoint::{EndpointTargetRef, RuntimeEndpointRef},
    ids::{
        ActivationEpoch, DescriptorRevision, InvocationId, JobId, NamespacedKind, RuntimeArtifactId,
    },
    schema::{SchemaRef, SchemaVersion},
};
use deckweave_host::{RuntimeCommand, RuntimeEvent, RuntimeEventKind};
use deckweave_protocol::v1::{
    self, CancelRequest, EndpointPath, InvocationRequest, OpaquePayload, SchemaReference,
    StructuredError, TerminalOutcome, ValueWrite, endpoint_path, invocation_reply,
    protocol_envelope, terminal_outcome,
};
use deckweave_registry::SessionIdentity;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn locale_context(
    value: &deckweave_domain::localization::LocaleContext,
) -> v1::LocaleContext {
    v1::LocaleContext {
        revision: value.revision,
        requested_locale: value.requested_locale.clone(),
        time_zone: value.time_zone.clone(),
    }
}

#[derive(Default)]
pub(super) struct ReplyContracts {
    pending: BTreeMap<String, SchemaRef>,
    completed: BTreeMap<String, SchemaRef>,
    order: std::collections::VecDeque<String>,
}

impl ReplyContracts {
    pub(super) fn track(&mut self, command: &RuntimeCommand) -> Result<(), HostRuntimeError> {
        match command {
            RuntimeCommand::Invoke { request, operation } => {
                if self.pending.len() >= 4096 {
                    return Err(HostRuntimeError::Protocol(
                        "reply contract limit exceeded".into(),
                    ));
                }
                self.pending.insert(
                    request.invocation_id.to_string(),
                    operation.result_schema.clone(),
                );
            }
            RuntimeCommand::Cancel { invocation_id, .. } => {
                self.finish(invocation_id.as_str());
            }
            _ => {}
        }
        Ok(())
    }
    fn schema(&self, id: &str) -> Result<&SchemaRef, HostRuntimeError> {
        self.pending
            .get(id)
            .or_else(|| self.completed.get(id))
            .ok_or_else(|| HostRuntimeError::Protocol("reply for unknown invocation".into()))
    }
    pub(super) fn finish(&mut self, id: &str) {
        if let Some(schema) = self.pending.remove(id) {
            while self.order.len() >= 256 {
                if let Some(old) = self.order.pop_front() {
                    self.completed.remove(&old);
                }
            }
            self.completed.insert(id.into(), schema);
            self.order.push_back(id.into());
        }
    }
}

pub(super) fn command_body(
    command: &RuntimeCommand,
) -> Result<Option<protocol_envelope::Body>, HostRuntimeError> {
    Ok(match command {
        RuntimeCommand::Invoke { request, operation } => {
            Some(protocol_envelope::Body::Invoke(InvocationRequest {
                configured_instance_id: request
                    .configured_instance_id
                    .as_ref()
                    .map(ToString::to_string),
                invocation_id: request.invocation_id.to_string(),
                run_id: request.execution_id.to_string(),
                operation_id: operation.operation_id.to_string(),
                activation_epoch: request.activation_epoch.get(),
                descriptor_revision: request.target.descriptor_revision.get(),
                deadline_host_ticks: request.deadline_host_ticks,
                remaining_budget_ms: None,
                idempotency_key: request.idempotency_key.clone(),
                input: Some(payload(&operation.input_schema, &request.input)?),
                target: Some(endpoint_path(&request.target)),
            }))
        }
        RuntimeCommand::Feedback {
            configured_instance_id,
            run_id,
            graph_node_id,
            activation_epoch,
            target,
            schema,
            value,
        } => Some(protocol_envelope::Body::ValueWrite(ValueWrite {
            configured_instance_id: configured_instance_id.as_ref().map(ToString::to_string),
            run_id: run_id.to_string(),
            graph_node_id: graph_node_id.to_string(),
            activation_epoch: activation_epoch.get(),
            descriptor_revision: target.descriptor_revision.get(),
            target: Some(endpoint_path(target)),
            value: Some(payload(schema, value)?),
        })),
        RuntimeCommand::Cancel {
            invocation_id,
            job_id,
            reason,
            ..
        } => Some(protocol_envelope::Body::Cancel(CancelRequest {
            invocation_id: invocation_id.to_string(),
            job_id: job_id.as_ref().map(ToString::to_string),
            reason: reason.clone(),
        })),
        RuntimeCommand::CleanupRun {
            activation_epoch,
            run_id,
            reason,
        } => Some(protocol_envelope::Body::OwnerCleanup(v1::OwnerCleanup {
            cleanup_id: format!("{}:{}", activation_epoch.get(), run_id),
            activation_epoch: activation_epoch.get(),
            run_id: Some(run_id.to_string()),
            reason: reason.clone(),
        })),
        RuntimeCommand::CleanupAll {
            activation_epoch,
            reason,
        } => Some(protocol_envelope::Body::OwnerCleanup(v1::OwnerCleanup {
            cleanup_id: format!("{}:all", activation_epoch.get()),
            activation_epoch: activation_epoch.get(),
            run_id: None,
            reason: reason.clone(),
        })),
    })
}

pub(super) fn event(
    session: &SessionIdentity,
    descriptor_revision: DescriptorRevision,
    activation_epoch: ActivationEpoch,
    body: &protocol_envelope::Body,
    nodes: &[deckweave_domain::capability::NodeDescriptor],
    replies: &mut ReplyContracts,
) -> Result<Option<RuntimeEvent>, HostRuntimeError> {
    let kind = match body {
        protocol_envelope::Body::Signal(signal) => {
            let source = normalize_signal_source(session, descriptor_revision, signal)
                .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
            let port = super::binding::find_port(nodes, &source.stable).ok_or_else(|| {
                HostRuntimeError::Protocol("signal endpoint is not declared".into())
            })?;
            if port.direction != deckweave_domain::capability::PortDirection::Output {
                return Err(HostRuntimeError::Protocol("signal on input port".into()));
            }
            let payload = decode_payload(signal.payload.as_ref(), &port.schema)?;
            use deckweave_domain::capability::PortSemantics;
            let coalescible = port.semantics == PortSemantics::State
                || (port.semantics == PortSemantics::Value
                    && port.tags.contains("org.deckweave.delivery.latest"));
            RuntimeEventKind::Signal {
                source,
                payload,
                source_ticks: signal.captured_ticks,
                reliability: if coalescible {
                    IngressReliability::Coalescible
                } else {
                    IngressReliability::Reliable
                },
            }
        }
        protocol_envelope::Body::InvocationReply(reply) => {
            let parsed = invocation_reply(reply, replies.schema(&reply.invocation_id)?)?;
            if !matches!(parsed, InvocationReply::AcceptedJob { .. }) {
                replies.finish(&reply.invocation_id);
            }
            RuntimeEventKind::Invocation(parsed)
        }
        protocol_envelope::Body::JobOutcome(outcome) => {
            let parsed = terminal_reply(outcome, replies.schema(&outcome.invocation_id)?)?;
            replies.finish(&outcome.invocation_id);
            RuntimeEventKind::Invocation(parsed)
        }
        _ => return Ok(None),
    };
    Ok(Some(RuntimeEvent {
        activation_epoch,
        kind,
    }))
}

fn invocation_reply(
    reply: &v1::InvocationReply,
    expected: &SchemaRef,
) -> Result<InvocationReply, HostRuntimeError> {
    let invocation_id = InvocationId::try_new(reply.invocation_id.clone())
        .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
    match reply
        .reply
        .as_ref()
        .ok_or_else(|| HostRuntimeError::Protocol("invocation reply has no body".into()))?
    {
        invocation_reply::Reply::Terminal(outcome) => {
            if outcome.invocation_id != reply.invocation_id {
                return Err(HostRuntimeError::Protocol(
                    "terminal invocation identity mismatch".into(),
                ));
            }
            terminal_reply(outcome, expected)
        }
        invocation_reply::Reply::AcceptedJobId(job_id) => Ok(InvocationReply::AcceptedJob {
            invocation_id,
            job_id: JobId::try_new(job_id.clone())
                .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?,
        }),
        invocation_reply::Reply::Rejected(error) => Ok(InvocationReply::Rejected {
            invocation_id,
            error: domain_error(error)?,
        }),
    }
}

fn terminal_reply(
    outcome: &TerminalOutcome,
    expected: &SchemaRef,
) -> Result<InvocationReply, HostRuntimeError> {
    let invocation_id = InvocationId::try_new(outcome.invocation_id.clone())
        .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
    let result = outcome
        .result
        .as_ref()
        .ok_or_else(|| HostRuntimeError::Protocol("terminal outcome has no result".into()))?;
    let outcome = match result {
        terminal_outcome::Result::Succeeded(payload) => CommandOutcome::Succeeded {
            result: decode_payload(Some(payload), expected)?,
        },
        terminal_outcome::Result::Failed(error) => CommandOutcome::Failed {
            error: domain_error(error)?,
        },
        terminal_outcome::Result::Cancelled(error) => CommandOutcome::Cancelled {
            error: domain_error(error)?,
            partial_effects: vec![],
        },
        terminal_outcome::Result::UnknownOutcome(error) => CommandOutcome::UnknownOutcome {
            error: domain_error(error)?,
            known_effects: vec![],
        },
    };
    Ok(InvocationReply::Terminal {
        invocation_id,
        outcome,
    })
}

pub(super) fn unknown_outcome(command: &RuntimeCommand, code: &str) -> Option<RuntimeEventKind> {
    let RuntimeCommand::Invoke { request, .. } = command else {
        return None;
    };
    Some(RuntimeEventKind::Invocation(InvocationReply::Terminal {
        invocation_id: request.invocation_id.clone(),
        outcome: CommandOutcome::UnknownOutcome {
            error: DeckWeaveError {
                category: ErrorCategory::UnknownOutcome,
                code: code.into(),
                message_key: format!("runtime.{code}"),
                details: BTreeMap::new(),
                diagnostic_attachment_id: None,
            },
            known_effects: vec![],
        },
    }))
}

fn payload(schema: &SchemaRef, value: &Value) -> Result<OpaquePayload, HostRuntimeError> {
    Ok(OpaquePayload {
        schema: Some(SchemaReference {
            id: schema.id.to_string(),
            major: schema.version.major,
            minor: schema.version.minor,
        }),
        value: serde_json::to_vec(value)
            .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?,
    })
}

fn decode_payload(
    payload: Option<&OpaquePayload>,
    expected: &SchemaRef,
) -> Result<Value, HostRuntimeError> {
    let payload =
        payload.ok_or_else(|| HostRuntimeError::Protocol("wire payload is missing".into()))?;
    let schema = payload
        .schema
        .as_ref()
        .ok_or_else(|| HostRuntimeError::Protocol("wire schema is missing".into()))?;
    if schema_reference(schema)? != *expected {
        return Err(HostRuntimeError::Protocol("wire schema mismatch".into()));
    }
    if payload.value.len() > 1024 * 1024 {
        return Err(HostRuntimeError::Protocol(
            "payload byte limit exceeded".into(),
        ));
    }
    let value = serde_json::from_slice(&payload.value)
        .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
    if !deckweave_domain::schema::bounded_json(&value) {
        return Err(HostRuntimeError::Protocol(
            "payload complexity exceeded".into(),
        ));
    }
    Ok(value)
}

fn endpoint_path(endpoint: &RuntimeEndpointRef) -> EndpointPath {
    let target = match &endpoint.stable.target {
        EndpointTargetRef::Device {
            device_identity, ..
        } => endpoint_path::Target::DeviceIdentity(device_identity.to_string()),
        EndpointTargetRef::Service { service_id, .. } => {
            endpoint_path::Target::ServiceId(service_id.to_string())
        }
    };
    let provider_id = match &endpoint.stable.target {
        EndpointTargetRef::Device { provider_id, .. }
        | EndpointTargetRef::Service { provider_id, .. } => provider_id.to_string(),
    };
    EndpointPath {
        node_id: endpoint.stable.node_id.to_string(),
        provider_id,
        target: Some(target),
        element_id: endpoint.stable.port.element_id.to_string(),
        capability_id: endpoint.stable.port.capability_id.to_string(),
        port_id: endpoint.stable.port.port_id.to_string(),
    }
}

fn domain_error(error: &StructuredError) -> Result<DeckWeaveError, HostRuntimeError> {
    let category = match v1::ErrorCategory::try_from(error.category)
        .unwrap_or(v1::ErrorCategory::Unspecified)
    {
        v1::ErrorCategory::InvalidInput => ErrorCategory::InvalidInput,
        v1::ErrorCategory::Unsupported => ErrorCategory::Unsupported,
        v1::ErrorCategory::Unavailable => ErrorCategory::Unavailable,
        v1::ErrorCategory::PermissionDenied => ErrorCategory::PermissionDenied,
        v1::ErrorCategory::ResourceBusy => ErrorCategory::ResourceBusy,
        v1::ErrorCategory::DeadlineExceeded => ErrorCategory::DeadlineExceeded,
        v1::ErrorCategory::Cancelled => ErrorCategory::Cancelled,
        v1::ErrorCategory::TransientExternal => ErrorCategory::TransientExternal,
        v1::ErrorCategory::PermanentExternal => ErrorCategory::PermanentExternal,
        v1::ErrorCategory::UnknownOutcome => ErrorCategory::UnknownOutcome,
        v1::ErrorCategory::Internal | v1::ErrorCategory::Unspecified => ErrorCategory::Internal,
    };
    let diagnostic_attachment_id = error
        .diagnostic_attachment_id
        .as_ref()
        .map(|value| RuntimeArtifactId::try_new(value.clone()))
        .transpose()
        .map_err(|parse| HostRuntimeError::Protocol(parse.to_string()))?;
    Ok(DeckWeaveError {
        category,
        code: error.code.clone(),
        message_key: error.message_key.clone(),
        details: error
            .details
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
        diagnostic_attachment_id,
    })
}

fn schema_reference(schema: &SchemaReference) -> Result<SchemaRef, HostRuntimeError> {
    Ok(SchemaRef {
        id: NamespacedKind::try_new(schema.id.clone())
            .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?,
        version: SchemaVersion {
            major: schema.major,
            minor: schema.minor,
        },
    })
}

#[cfg(test)]
mod tests;
