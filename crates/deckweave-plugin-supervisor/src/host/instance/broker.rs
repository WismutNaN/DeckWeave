// SPDX-License-Identifier: GPL-3.0-or-later
//! One-use tickets bind plugin requests to the exact host-authorized invocation.
use super::*;
use deckweave_domain::{ids::ActivationEpoch, security::PermissionAccessMode};
use deckweave_host::RuntimeCommand;
use deckweave_platform::{UserProcessFailure, UserProcessRequest};
use deckweave_protocol::v1::{
    ErrorCategory, HostProcessReply, HostProcessRequest, StructuredError, UserProcessReceipt,
    host_process_reply,
};
#[cfg(test)]
mod tests;

struct Ticket {
    epoch: u64,
    operation: String,
    schema: deckweave_domain::schema::SchemaRef,
    input: serde_json::Value,
    deadline: u64,
    run_id: String,
}
#[derive(Default)]
pub(super) struct Tickets {
    pending: BTreeMap<String, Ticket>,
}

impl Tickets {
    pub(super) fn revoke_all(&mut self) {
        self.pending.clear();
    }
    pub(super) fn track(
        &mut self,
        command: &RuntimeCommand,
        now: u64,
    ) -> Result<(), HostRuntimeError> {
        self.pending.retain(|_, t| t.deadline > now);
        match command {
            RuntimeCommand::Invoke { request, operation } => {
                let scope = match operation.operation_id.as_str() {
                    "launch-process" => "windows.process.launch",
                    "open-target" => "windows.shell.open",
                    _ => return Ok(()),
                };
                if !operation
                    .effects
                    .iter()
                    .any(|effect| effect.as_str() == scope)
                {
                    return Ok(());
                }
                if self.pending.len() >= 128
                    || self.pending.contains_key(request.invocation_id.as_str())
                {
                    return Err(HostRuntimeError::Protocol(
                        "process broker ticket budget/replay".into(),
                    ));
                }
                self.pending.insert(
                    request.invocation_id.to_string(),
                    Ticket {
                        epoch: request.activation_epoch.get(),
                        operation: operation.operation_id.to_string(),
                        schema: operation.input_schema.clone(),
                        input: request.input.clone(),
                        deadline: request.deadline_host_ticks,
                        run_id: request.execution_id.to_string(),
                    },
                );
            }
            RuntimeCommand::Cancel { invocation_id, .. } => {
                self.pending.remove(invocation_id.as_str());
            }
            RuntimeCommand::CleanupAll { .. } => self.pending.clear(),
            RuntimeCommand::CleanupRun { run_id, .. } => {
                self.pending.retain(|_, t| t.run_id != run_id.as_str())
            }
            _ => {}
        }
        Ok(())
    }

    fn claim(
        &mut self,
        request: &HostProcessRequest,
        epoch: u64,
        now: u64,
    ) -> Result<(UserProcessRequest, u64), UserProcessFailure> {
        let ticket = self
            .pending
            .remove(&request.invocation_id)
            .ok_or(UserProcessFailure::Rejected)?;
        if ticket.deadline <= now {
            return Err(UserProcessFailure::Deadline);
        }
        let input = request
            .input
            .as_ref()
            .ok_or(UserProcessFailure::InvalidInput)?;
        if request.activation_epoch != epoch
            || ticket.epoch != epoch
            || ticket.operation != request.operation_id
            || input.value.len() > 65536
            || !input.schema.as_ref().is_some_and(|s| {
                s.id == ticket.schema.id.as_str()
                    && s.major == ticket.schema.version.major
                    && s.minor == ticket.schema.version.minor
            })
            || serde_json::from_slice::<serde_json::Value>(&input.value)
                .ok()
                .as_ref()
                != Some(&ticket.input)
        {
            return Err(UserProcessFailure::Rejected);
        }
        let effect = match (ticket.operation.as_str(), ticket.schema.id.as_str()) {
            ("launch-process", "org.deckweave.windows.app_target") => UserProcessRequest::Launch(
                serde_json::from_value(ticket.input)
                    .map_err(|_| UserProcessFailure::InvalidInput)?,
            ),
            ("open-target", "org.deckweave.windows.shell_target") => UserProcessRequest::Open(
                serde_json::from_value(ticket.input)
                    .map_err(|_| UserProcessFailure::InvalidInput)?,
            ),
            _ => return Err(UserProcessFailure::Rejected),
        };
        effect.validate()?;
        Ok((effect, ticket.deadline))
    }
}

pub(super) fn serve(
    instance: &mut RuntimeInstance,
    request: &HostProcessRequest,
    epoch: ActivationEpoch,
    config: &HostRuntimeConfig,
    clock: Instant,
) -> Result<(), HostRuntimeError> {
    let result = execute(instance, request, epoch, config, clock);
    let result = match result {
        Ok(receipt) => host_process_reply::Result::Succeeded(UserProcessReceipt {
            process_id: receipt.process_id,
        }),
        Err(UserProcessFailure::UnknownOutcome) => {
            host_process_reply::Result::UnknownOutcome(error(UserProcessFailure::UnknownOutcome))
        }
        Err(failure) => host_process_reply::Result::Rejected(error(failure)),
    };
    instance.process.send(&envelope(
        instance.generation,
        &mut instance.next_message_id,
        protocol_envelope::Body::HostProcessReply(HostProcessReply {
            invocation_id: request.invocation_id.clone(),
            result: Some(result),
        }),
    )?)?;
    Ok(())
}

fn execute(
    instance: &mut RuntimeInstance,
    request: &HostProcessRequest,
    epoch: ActivationEpoch,
    config: &HostRuntimeConfig,
    clock: Instant,
) -> Result<deckweave_platform::UserProcessReceipt, UserProcessFailure> {
    if !instance.controller.session().is_some_and(|s| {
        s.negotiated_features
            .get(deckweave_protocol::FEATURE_HOST_PROCESS_BROKER)
            == Some(&1)
    }) {
        return Err(UserProcessFailure::Unsupported);
    }
    let launcher = config
        .user_process_launcher
        .as_ref()
        .ok_or(UserProcessFailure::Unsupported)?;
    let (effect, deadline) =
        instance
            .broker_tickets
            .claim(request, epoch.get(), elapsed_millis(clock))?;
    let (scope, selector) = match &effect {
        UserProcessRequest::Launch(_) => (
            "windows.process.launch",
            serde_json::json!({"mode":"exact_executable"}),
        ),
        UserProcessRequest::Open(_) => (
            "windows.shell.open",
            serde_json::json!({"mode":"file_or_uri"}),
        ),
    };
    let grant = instance
        .controller
        .granted_request(scope)
        .ok_or(UserProcessFailure::Rejected)?;
    if grant.principal_id != instance.principal
        || grant.resource_selector != selector
        || grant.access_mode != PermissionAccessMode::Execute
    {
        return Err(UserProcessFailure::Rejected);
    }
    instance
        .authorization
        .authorize(grant)
        .map_err(|_| UserProcessFailure::Rejected)?;
    let remaining = deadline
        .checked_sub(elapsed_millis(clock))
        .filter(|v| *v > 0)
        .ok_or(UserProcessFailure::Deadline)?;
    launcher.execute(&effect, Instant::now() + Duration::from_millis(remaining))
}

fn error(failure: UserProcessFailure) -> StructuredError {
    let (category, code) = match failure {
        UserProcessFailure::InvalidInput => (ErrorCategory::InvalidInput, "invalid_input"),
        UserProcessFailure::Unsupported => (ErrorCategory::Unsupported, "broker_unavailable"),
        UserProcessFailure::Rejected => (ErrorCategory::PermissionDenied, "launch_rejected"),
        UserProcessFailure::Deadline => (ErrorCategory::DeadlineExceeded, "deadline_exceeded"),
        UserProcessFailure::Busy => (ErrorCategory::ResourceBusy, "broker_busy"),
        UserProcessFailure::UnknownOutcome => (ErrorCategory::Unavailable, "unknown_outcome"),
    };
    StructuredError {
        category: category as i32,
        code: code.into(),
        message_key: format!("platform.process.{code}"),
        details: Default::default(),
        diagnostic_attachment_id: None,
    }
}
