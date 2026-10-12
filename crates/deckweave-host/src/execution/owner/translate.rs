// SPDX-License-Identifier: GPL-3.0-or-later
//! Translation of scheduler intents into runtime-addressed commands.

use super::{ActiveExecution, LiveExecutionError};
use crate::RuntimeCommand;
use deckweave_automation::SchedulerIntent;
use deckweave_domain::{
    automation::CommandEnvelope, endpoint::RuntimeEndpointRef, ids::GraphNodeId,
};

pub(super) fn translate_intent(
    active: &mut ActiveExecution,
    intent: SchedulerIntent,
) -> Result<RuntimeCommand, LiveExecutionError> {
    match intent {
        SchedulerIntent::Invoke {
            invocation_id,
            run_id,
            node_id,
            input,
            deadline_ticks,
            ..
        } => {
            let target = binding(active, &node_id)?;
            let operation = active
                .graph
                .endpoint_contracts
                .iter()
                .find(|contract| contract.node_id == node_id)
                .and_then(|contract| contract.operation.clone())
                .ok_or_else(|| LiveExecutionError::MissingBinding(node_id.clone()))?;
            active
                .invocation_routes
                .insert(invocation_id.clone(), target.clone());
            Ok(RuntimeCommand::Invoke {
                request: Box::new(CommandEnvelope {
                    invocation_id,
                    execution_id: run_id,
                    graph_node_id: node_id,
                    activation_epoch: active.epoch,
                    target,
                    input,
                    deadline_host_ticks: deadline_ticks,
                    idempotency_key: None,
                    extensions: Default::default(),
                }),
                operation,
            })
        }
        SchedulerIntent::Feedback {
            run_id,
            node_id,
            value,
            ..
        } => {
            let target = binding(active, &node_id)?;
            let schema = active
                .graph
                .endpoint_contracts
                .iter()
                .find(|contract| contract.node_id == node_id)
                .map(|contract| contract.port.schema.clone())
                .ok_or_else(|| LiveExecutionError::MissingBinding(node_id.clone()))?;
            Ok(RuntimeCommand::Feedback {
                run_id,
                graph_node_id: node_id,
                activation_epoch: active.epoch,
                target,
                schema,
                value,
            })
        }
        SchedulerIntent::CancelInvocation {
            invocation_id,
            job_id,
            reason,
        } => {
            let target = active
                .invocation_routes
                .remove(&invocation_id)
                .ok_or_else(|| {
                    LiveExecutionError::Scheduler("cancel target is no longer owned".into())
                })?;
            Ok(RuntimeCommand::Cancel {
                activation_epoch: active.epoch,
                invocation_id,
                job_id,
                target,
                reason,
            })
        }
        SchedulerIntent::CleanupRun { run_id, reason } => Ok(RuntimeCommand::CleanupRun {
            activation_epoch: active.epoch,
            run_id,
            reason,
        }),
        SchedulerIntent::CleanupAll { reason } => {
            active.invocation_routes.clear();
            Ok(RuntimeCommand::CleanupAll {
                activation_epoch: active.epoch,
                reason,
            })
        }
    }
}

pub(super) fn translate_cleanup(
    active: &mut ActiveExecution,
    intent: SchedulerIntent,
) -> Result<RuntimeCommand, LiveExecutionError> {
    translate_intent(active, intent)
}

fn binding(
    active: &ActiveExecution,
    node_id: &GraphNodeId,
) -> Result<RuntimeEndpointRef, LiveExecutionError> {
    active
        .bindings
        .get(node_id)
        .cloned()
        .ok_or_else(|| LiveExecutionError::MissingBinding(node_id.clone()))
}
