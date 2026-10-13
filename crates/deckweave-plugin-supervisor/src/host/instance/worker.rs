// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded command worker and inbound runtime monitor.

use super::{RuntimeInstance, elapsed_millis, envelope, wire};
mod events;
mod shutdown;
use crate::{
    host::{
        HostRuntimeConfig, HostRuntimeError,
        health::{RuntimeHealthStatus, RuntimeHealthStore},
    },
    process::ProcessError,
    supervisor::SupervisorAction,
};
use deckweave_domain::ids::{
    ActivationEpoch, GrantRevision, PackageId, PluginSessionId, RuntimeGroupId, SessionGeneration,
};
use deckweave_host::{RuntimeCommand, RuntimeEvent, RuntimeEventKind, RuntimeEventSink};
use deckweave_protocol::v1::protocol_envelope;
use deckweave_registry::CapabilityRegistry;
use events::{process_stopped, receive};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct WorkerKey {
    pub(in crate::host) package_id: PackageId,
    pub(in crate::host) runtime_group_id: RuntimeGroupId,
    pub(super) plugin_session_id: PluginSessionId,
    pub(in crate::host) generation: SessionGeneration,
}

impl WorkerKey {
    pub(crate) fn from_endpoint(endpoint: &deckweave_domain::endpoint::RuntimeEndpointRef) -> Self {
        Self {
            package_id: endpoint.stable.package_id.clone(),
            runtime_group_id: endpoint.runtime_group_id.clone(),
            plugin_session_id: endpoint.plugin_session_id.clone(),
            generation: endpoint.session_generation,
        }
    }
}

pub(crate) enum WorkerCommand {
    Start,
    Locale(deckweave_domain::localization::LocaleContext),
    Dispatch(Box<RuntimeCommand>),
    Shutdown(mpsc::SyncSender<Result<(), HostRuntimeError>>),
}

impl From<RuntimeCommand> for WorkerCommand {
    fn from(command: RuntimeCommand) -> Self {
        Self::Dispatch(Box::new(command))
    }
}

pub(crate) struct ActiveWorker {
    failed_release: Arc<std::sync::atomic::AtomicBool>,
    sender: mpsc::SyncSender<WorkerCommand>,
    key: WorkerKey,
    grant_revision: GrantRevision,
    features: BTreeMap<String, u32>,
    join: Option<thread::JoinHandle<Result<(), HostRuntimeError>>>,
}

impl ActiveWorker {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn(
        instance: RuntimeInstance,
        registry: Arc<Mutex<CapabilityRegistry>>,
        config: HostRuntimeConfig,
        clock: Instant,
        activation_epoch: ActivationEpoch,
        sink: Arc<dyn RuntimeEventSink>,
        health: RuntimeHealthStore,
    ) -> Result<Self, HostRuntimeError> {
        let (key, features) = instance.worker_identity()?;
        let grant_revision = instance.grant_revision;
        let failed_release = instance.failed_release.clone();
        health.update(instance.health_entry(RuntimeHealthStatus::Active, None));
        let (sender, receiver) = mpsc::sync_channel(config.process.maximum_outbound_frames);
        let join = thread::spawn(move || {
            monitor(
                instance,
                registry,
                config,
                clock,
                activation_epoch,
                sink,
                health,
                receiver,
            )
        });
        Ok(Self {
            failed_release,
            sender,
            key,
            grant_revision,
            features,
            join: Some(join),
        })
    }

    pub(crate) fn sender(&self) -> mpsc::SyncSender<WorkerCommand> {
        self.sender.clone()
    }
    pub(crate) fn is_alive(&self) -> bool {
        self.join.as_ref().is_some_and(|join| !join.is_finished())
            && !self
                .failed_release
                .load(std::sync::atomic::Ordering::SeqCst)
    }
    pub(crate) fn failed_release(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.failed_release.clone()
    }
    pub(crate) fn key(&self) -> &WorkerKey {
        &self.key
    }
    pub(crate) fn grant_revision(&self) -> GrantRevision {
        self.grant_revision
    }
    pub(crate) fn features(&self) -> &BTreeMap<String, u32> {
        &self.features
    }
}

impl Drop for ActiveWorker {
    fn drop(&mut self) {
        let (done, _receiver) = mpsc::sync_channel(1);
        let _ignored = self.sender.try_send(WorkerCommand::Shutdown(done));
    }
}

#[allow(clippy::too_many_arguments)]
fn monitor(
    mut instance: RuntimeInstance,
    registry: Arc<Mutex<CapabilityRegistry>>,
    config: HostRuntimeConfig,
    clock: Instant,
    activation_epoch: ActivationEpoch,
    sink: Arc<dyn RuntimeEventSink>,
    health: RuntimeHealthStore,
    commands: mpsc::Receiver<WorkerCommand>,
) -> Result<(), HostRuntimeError> {
    let mut started = false;
    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                WorkerCommand::Start => {
                    if instance.controller.session().is_some_and(|s| {
                        s.negotiated_features
                            .get(deckweave_protocol::FEATURE_ACTIVATION_COMMIT)
                            == Some(&1)
                    }) {
                        let sent = envelope(
                            instance.generation,
                            &mut instance.next_message_id,
                            protocol_envelope::Body::ActivationCommitted(
                                deckweave_protocol::v1::ActivationCommitted {
                                    activation_epoch: activation_epoch.get(),
                                    host_ticks: elapsed_millis(clock),
                                },
                            ),
                        )
                        .and_then(|message| instance.process.send(&message).map_err(Into::into));
                        if let Err(error) = sent {
                            health.update(instance.health_entry(
                                RuntimeHealthStatus::Unavailable,
                                Some(error.to_string()),
                            ));
                            return Err(error);
                        }
                    }
                    started = true;
                }
                WorkerCommand::Locale(context) => {
                    if instance.controller.session().is_some_and(|s| {
                        s.negotiated_features
                            .get(deckweave_protocol::FEATURE_LOCALE_CONTEXT)
                            .copied()
                            == Some(1)
                    }) {
                        let sent = envelope(
                            instance.generation,
                            &mut instance.next_message_id,
                            protocol_envelope::Body::LocaleContext(wire::locale_context(&context)),
                        )
                        .and_then(|message| instance.process.send(&message).map_err(Into::into));
                        if let Err(error) = sent {
                            health.update(instance.health_entry(
                                RuntimeHealthStatus::Degraded,
                                Some(error.to_string()),
                            ));
                        }
                    }
                }
                WorkerCommand::Shutdown(done) => {
                    let result = shutdown::drain_and_stop(
                        instance,
                        &registry,
                        activation_epoch,
                        sink.as_ref(),
                        &health,
                        &config,
                        clock,
                    );
                    let _ignored = done.try_send(
                        result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|error| HostRuntimeError::Shutdown(error.to_string())),
                    );
                    return result;
                }
                WorkerCommand::Dispatch(command) => dispatch(
                    &mut instance,
                    command.as_ref(),
                    activation_epoch,
                    sink.as_ref(),
                    &health,
                    clock,
                ),
            }
        }
        if !started {
            thread::sleep(config.monitor_interval.min(Duration::from_millis(10)));
            continue;
        }
        if receive(
            &mut instance,
            &registry,
            activation_epoch,
            sink.as_ref(),
            &health,
            &config,
            clock,
        ) {
            return Err(HostRuntimeError::Shutdown(
                "runtime transport failed".into(),
            ));
        }
        let expired = instance
            .cleanups
            .iter()
            .filter(|(_, deadline)| Instant::now() >= **deadline)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for cleanup_id in expired {
            instance
                .failed_release
                .store(true, std::sync::atomic::Ordering::SeqCst);
            instance.cleanup_failed = true;
            instance.cleanups.remove(&cleanup_id);
            sink.publish(RuntimeEvent {
                activation_epoch,
                kind: RuntimeEventKind::CleanupReceipt {
                    cleanup_id,
                    released: false,
                },
            });
            health.update(instance.health_entry(
                RuntimeHealthStatus::Degraded,
                Some("owner cleanup receipt timeout".into()),
            ));
        }
        if process_stopped(
            &mut instance,
            &registry,
            activation_epoch,
            sink.as_ref(),
            &health,
            clock,
        ) {
            // Fault publication synchronously queues owner release intents. Drain those
            // before stopping the process; ordinary work is no longer admitted.
            let waiters = shutdown::release_queued(&commands, |command| {
                dispatch(
                    &mut instance,
                    command,
                    activation_epoch,
                    sink.as_ref(),
                    &health,
                    clock,
                );
            });
            let result = shutdown::drain_and_stop(
                instance,
                &registry,
                activation_epoch,
                sink.as_ref(),
                &health,
                &config,
                clock,
            );
            for done in waiters {
                let _ = done.try_send(
                    result
                        .as_ref()
                        .map(|_| ())
                        .map_err(|e| HostRuntimeError::Shutdown(e.to_string())),
                );
            }
            return result;
        }
    }
}

fn dispatch(
    instance: &mut RuntimeInstance,
    command: &RuntimeCommand,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
    health: &RuntimeHealthStore,
    clock: Instant,
) {
    if let Err(error) = dispatch_command(instance, command, elapsed_millis(clock)) {
        if let Ok(Some(protocol_envelope::Body::OwnerCleanup(cleanup))) =
            wire::command_body(command)
        {
            instance.cleanup_failed = true;
            instance
                .failed_release
                .store(true, std::sync::atomic::Ordering::SeqCst);
            instance.cleanups.remove(&cleanup.cleanup_id);
            sink.publish(RuntimeEvent {
                activation_epoch,
                kind: RuntimeEventKind::CleanupReceipt {
                    cleanup_id: cleanup.cleanup_id,
                    released: false,
                },
            });
        }
        if let Some(kind) = wire::unknown_outcome(command, "transport_send_failed") {
            sink.publish(RuntimeEvent {
                activation_epoch,
                kind,
            });
        }
        health
            .update(instance.health_entry(RuntimeHealthStatus::Degraded, Some(error.to_string())));
    }
}

fn dispatch_command(
    instance: &mut RuntimeInstance,
    command: &RuntimeCommand,
    now: u64,
) -> Result<(), HostRuntimeError> {
    let Some(mut body) = wire::command_body(command)? else {
        return Ok(());
    };
    if let protocol_envelope::Body::Invoke(request) = &mut body {
        if request.deadline_host_ticks <= now {
            return Err(HostRuntimeError::Protocol(
                "invocation expired before transport admission".into(),
            ));
        }
        request.remaining_budget_ms = Some(request.deadline_host_ticks.saturating_sub(now));
    }
    if let protocol_envelope::Body::OwnerCleanup(request) = &mut body {
        if instance.cleanups.len() >= 4096 {
            return Err(HostRuntimeError::Protocol("cleanup budget exceeded".into()));
        }
        request.cleanup_id = format!(
            "{}:{}:{}",
            instance.generation, instance.next_message_id, request.cleanup_id
        );
        instance.cleanups.insert(
            request.cleanup_id.clone(),
            Instant::now() + Duration::from_secs(2),
        );
    }
    let request = envelope(instance.generation, &mut instance.next_message_id, body)?;
    instance.broker_tickets.track(command, now)?;
    instance.replies.track(command)?;
    if let Err(error) = instance.process.send(&request) {
        if let RuntimeCommand::Invoke { request, .. } = command {
            instance.replies.finish(request.invocation_id.as_str());
        }
        return Err(error.into());
    }
    Ok(())
}
