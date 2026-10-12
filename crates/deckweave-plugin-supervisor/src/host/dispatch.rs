// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, fenced command routing to active runtime workers.

use super::instance::{ActiveWorker, WorkerCommand, WorkerKey};
use deckweave_application::AuthorizationStore;
use deckweave_domain::{
    automation::{DeckWeaveError, ErrorCategory},
    endpoint::RuntimeEndpointRef,
    ids::{ActivationEpoch, GrantRevision},
};
use deckweave_host::{RuntimeCommand, RuntimeDispatchFailure, RuntimeIntentDispatcher};
use deckweave_protocol::{FEATURE_INVOCATION_ENDPOINT_PATH, FEATURE_VALUE_WRITE_ENDPOINT_PATH};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
};

#[derive(Default)]
struct DispatchState {
    epoch: Option<ActivationEpoch>,
    closed: bool,
    workers: BTreeMap<WorkerKey, WorkerRoute>,
}

#[derive(Clone)]
struct WorkerRoute {
    sender: mpsc::SyncSender<WorkerCommand>,
    grant_revision: GrantRevision,
    features: BTreeMap<String, u32>,
}

pub(super) struct RuntimeDispatchHub {
    authorization: Arc<dyn AuthorizationStore>,
    state: Mutex<DispatchState>,
}

impl RuntimeDispatchHub {
    pub(super) fn new(authorization: Arc<dyn AuthorizationStore>) -> Self {
        Self {
            authorization,
            state: Mutex::new(DispatchState {
                closed: true,
                ..DispatchState::default()
            }),
        }
    }

    pub(super) fn activate(
        &self,
        epoch: Option<ActivationEpoch>,
        workers: &[ActiveWorker],
    ) -> Result<(), RuntimeDispatchFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| rejected("dispatch_state_poisoned"))?;
        state.workers = workers
            .iter()
            .map(|worker| {
                (
                    worker.key().clone(),
                    WorkerRoute {
                        sender: worker.sender(),
                        grant_revision: worker.grant_revision(),
                        features: worker.features().clone(),
                    },
                )
            })
            .collect();
        state.epoch = epoch;
        state.closed = epoch.is_none();
        Ok(())
    }

    pub(super) fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
        }
    }

    pub(super) fn reopen(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = state.epoch.is_none();
        }
    }

    pub(super) fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            state.epoch = None;
            state.workers.clear();
        }
    }
}

impl RuntimeIntentDispatcher for RuntimeDispatchHub {
    fn dispatch(&self, command: RuntimeCommand) -> Result<(), RuntimeDispatchFailure> {
        let Some((epoch, target, feature)) = route(&command) else {
            return Ok(());
        };
        let state = self
            .state
            .lock()
            .map_err(|_| rejected("dispatch_state_poisoned"))?;
        if state.closed || state.epoch != Some(epoch) {
            return Err(rejected("stale_or_closed_activation"));
        }
        let key = WorkerKey::from_endpoint(target);
        let worker = state
            .workers
            .get(&key)
            .ok_or_else(|| rejected("runtime_route_missing"))?;
        if worker.features.get(feature).copied() != Some(1) {
            return Err(rejected("required_endpoint_feature_not_negotiated"));
        }
        let current_revision = self
            .authorization
            .revision()
            .map_err(|_| rejected("grant_revision_unavailable"))?;
        if current_revision != worker.grant_revision {
            return Err(permission_denied("grant_revision_changed"));
        }
        worker
            .sender
            .try_send(command.into())
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => rejected("runtime_dispatch_queue_full"),
                mpsc::TrySendError::Disconnected(_) => rejected("runtime_worker_disconnected"),
            })
    }
}

fn route(command: &RuntimeCommand) -> Option<(ActivationEpoch, &RuntimeEndpointRef, &'static str)> {
    match command {
        RuntimeCommand::Invoke { request, .. } => Some((
            request.activation_epoch,
            &request.target,
            FEATURE_INVOCATION_ENDPOINT_PATH,
        )),
        RuntimeCommand::Feedback {
            activation_epoch,
            target,
            ..
        } => Some((*activation_epoch, target, FEATURE_VALUE_WRITE_ENDPOINT_PATH)),
        RuntimeCommand::Cancel {
            activation_epoch,
            target,
            ..
        } => Some((*activation_epoch, target, FEATURE_INVOCATION_ENDPOINT_PATH)),
        RuntimeCommand::CleanupRun { .. } | RuntimeCommand::CleanupAll { .. } => None,
    }
}

fn rejected(code: &str) -> RuntimeDispatchFailure {
    failure(ErrorCategory::Unavailable, code)
}

fn permission_denied(code: &str) -> RuntimeDispatchFailure {
    failure(ErrorCategory::PermissionDenied, code)
}

fn failure(category: ErrorCategory, code: &str) -> RuntimeDispatchFailure {
    RuntimeDispatchFailure {
        error: DeckWeaveError {
            category,
            code: code.into(),
            message_key: format!("runtime.{code}"),
            details: BTreeMap::<String, Value>::new(),
            diagnostic_attachment_id: None,
        },
        possibly_sent: false,
    }
}
