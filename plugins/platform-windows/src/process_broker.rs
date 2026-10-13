// SPDX-License-Identifier: GPL-3.0-or-later
//! Provider action semantics; only the host owns the external process lifetime.
use crate::{PlatformFailure, wire::WireSender};
use deckweave_protocol::v1::{
    HostProcessReply, HostProcessRequest, InvocationReply, InvocationRequest, OpaquePayload,
    SchemaReference, TerminalOutcome, endpoint_path, host_process_reply, invocation_reply,
    protocol_envelope, terminal_outcome,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct Broker {
    pub enabled: bool,
    pending: BTreeMap<String, Instant>,
}
impl Broker {
    pub fn invoke(
        &mut self,
        wire: &WireSender,
        request: &InvocationRequest,
    ) -> Result<bool, String> {
        let Some(target) = &request.target else {
            return Ok(false);
        };
        if target.node_id != "windows-control-node"
            || target.provider_id != "org.deckweave.platform.windows.control"
            || target.element_id != "applications"
            || !matches!(target.capability_id.as_str(), "launch" | "shell-open")
            || target.port_id != "invoke"
            || !matches!(&target.target,Some(endpoint_path::Target::ServiceId(id)) if id == "windows-platform")
        {
            return Ok(false);
        }
        let (operation, schema) = if target.capability_id == "launch" {
            ("launch-process", "org.deckweave.windows.app_target")
        } else {
            ("open-target", "org.deckweave.windows.shell_target")
        };
        let failure = if !self.enabled {
            Some(PlatformFailure::OsApiUnavailable)
        } else if request.remaining_budget_ms.unwrap_or(0) == 0 {
            Some(PlatformFailure::DeadlineExceeded)
        } else if request.operation_id != operation
            || !request.input.as_ref().is_some_and(|p| {
                p.value.len() <= 65536
                    && p.schema
                        .as_ref()
                        .is_some_and(|s| s.id == schema && s.major == 1 && s.minor == 0)
            })
        {
            Some(PlatformFailure::InvalidInput)
        } else if self.pending.len() >= 16 || self.pending.contains_key(&request.invocation_id) {
            Some(PlatformFailure::ResourceBusy)
        } else {
            None
        };
        if let Some(failure) = failure {
            wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
                invocation_id: request.invocation_id.clone(),
                reply: Some(invocation_reply::Reply::Rejected(failure.structured())),
            }))?;
        } else {
            self.pending.insert(
                request.invocation_id.clone(),
                Instant::now()
                    + Duration::from_millis(request.remaining_budget_ms.unwrap_or(0).min(5000)),
            );
            wire.send(protocol_envelope::Body::HostProcessRequest(
                HostProcessRequest {
                    invocation_id: request.invocation_id.clone(),
                    activation_epoch: request.activation_epoch,
                    operation_id: operation.into(),
                    input: request.input.clone(),
                },
            ))?;
        }
        Ok(true)
    }

    pub fn reply(&mut self, wire: &WireSender, reply: HostProcessReply) -> Result<(), String> {
        if self.pending.remove(&reply.invocation_id).is_none() {
            return Ok(());
        }
        let result = match reply.result {
            Some(host_process_reply::Result::Succeeded(receipt)) => {
                terminal_outcome::Result::Succeeded(OpaquePayload {
                    schema: Some(SchemaReference {
                        id: "org.deckweave.windows.launch_receipt".into(),
                        major: 1,
                        minor: 0,
                    }),
                    value: serde_json::to_vec(&serde_json::json!({"processId":receipt.process_id}))
                        .map_err(|e| e.to_string())?,
                })
            }
            Some(host_process_reply::Result::Rejected(error)) => {
                return wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
                    invocation_id: reply.invocation_id,
                    reply: Some(invocation_reply::Reply::Rejected(error)),
                }));
            }
            Some(host_process_reply::Result::UnknownOutcome(error)) => {
                terminal_outcome::Result::UnknownOutcome(error)
            }
            None => terminal_outcome::Result::UnknownOutcome(
                PlatformFailure::OsApiUnavailable.structured(),
            ),
        };
        terminal(wire, reply.invocation_id, result)
    }

    pub fn expire(&mut self, wire: &WireSender) -> Result<(), String> {
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, d)| Instant::now() >= **d)
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            self.pending.remove(&id);
            terminal(
                wire,
                id,
                terminal_outcome::Result::UnknownOutcome(
                    PlatformFailure::DeadlineExceeded.structured(),
                ),
            )?;
        }
        Ok(())
    }
}
fn terminal(wire: &WireSender, id: String, result: terminal_outcome::Result) -> Result<(), String> {
    wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
        invocation_id: id.clone(),
        reply: Some(invocation_reply::Reply::Terminal(TerminalOutcome {
            invocation_id: id,
            result: Some(result),
        })),
    }))
}
