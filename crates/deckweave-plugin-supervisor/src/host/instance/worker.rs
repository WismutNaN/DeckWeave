// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded command worker and inbound runtime monitor.

use super::{RuntimeInstance, elapsed_millis, envelope, wire};
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
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct WorkerKey {
    pub(super) package_id: PackageId,
    pub(super) runtime_group_id: RuntimeGroupId,
    pub(super) plugin_session_id: PluginSessionId,
    pub(super) generation: SessionGeneration,
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
    Dispatch(Box<RuntimeCommand>),
    Shutdown(mpsc::SyncSender<()>),
}

impl From<RuntimeCommand> for WorkerCommand {
    fn from(command: RuntimeCommand) -> Self {
        Self::Dispatch(Box::new(command))
    }
}

pub(crate) struct ActiveWorker {
    sender: mpsc::SyncSender<WorkerCommand>,
    key: WorkerKey,
    grant_revision: GrantRevision,
    features: BTreeMap<String, u32>,
    join: Option<thread::JoinHandle<()>>,
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
            );
        });
        Ok(Self {
            sender,
            key,
            grant_revision,
            features,
            join: Some(join),
        })
    }

    pub(crate) fn shutdown(mut self, timeout: Duration) -> Result<(), HostRuntimeError> {
        let (done, receiver) = mpsc::sync_channel(1);
        match self.sender.try_send(WorkerCommand::Shutdown(done)) {
            Ok(()) => receiver
                .recv_timeout(timeout)
                .map_err(|error| HostRuntimeError::Shutdown(error.to_string()))?,
            Err(mpsc::TrySendError::Disconnected(_)) => {}
            Err(error) => return Err(HostRuntimeError::Shutdown(error.to_string())),
        }
        if let Some(join) = self.join.take() {
            join.join()
                .map_err(|_| HostRuntimeError::Shutdown("runtime monitor panicked".into()))?;
        }
        Ok(())
    }

    pub(crate) fn sender(&self) -> mpsc::SyncSender<WorkerCommand> {
        self.sender.clone()
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
) {
    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                WorkerCommand::Shutdown(done) => {
                    health.update(instance.health_entry(RuntimeHealthStatus::Stopped, None));
                    let now = elapsed_millis(clock);
                    let result = registry.lock().ok().and_then(|mut registry| {
                        instance.stop(&mut registry, config.stop_timeout, now).ok()
                    });
                    let _ignored = done.try_send(());
                    let _ = result;
                    return;
                }
                WorkerCommand::Dispatch(command) => dispatch(
                    &mut instance,
                    command.as_ref(),
                    activation_epoch,
                    sink.as_ref(),
                    &health,
                ),
            }
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
            return;
        }
        if process_stopped(
            &mut instance,
            &registry,
            activation_epoch,
            sink.as_ref(),
            &health,
            clock,
        ) {
            return;
        }
    }
}

fn dispatch(
    instance: &mut RuntimeInstance,
    command: &RuntimeCommand,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
    health: &RuntimeHealthStore,
) {
    if let Err(error) = dispatch_command(instance, command) {
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

fn receive(
    instance: &mut RuntimeInstance,
    registry: &Arc<Mutex<CapabilityRegistry>>,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
    health: &RuntimeHealthStore,
    config: &HostRuntimeConfig,
    clock: Instant,
) -> bool {
    match instance.process.receive(config.monitor_interval) {
        Ok(envelope) => {
            if instance
                .controller
                .accept_envelope(&envelope, elapsed_millis(clock))
                .is_err()
            {
                return false;
            }
            let session = instance
                .controller
                .session()
                .map(|value| value.identity.clone());
            let Some((session, body)) = session.zip(envelope.body.as_ref()) else {
                return false;
            };
            match wire::event(
                &session,
                instance.descriptor_revision,
                activation_epoch,
                body,
            ) {
                Ok(Some(event)) => sink.publish(event),
                Ok(None)
                    if matches!(
                        body,
                        protocol_envelope::Body::DescriptorSnapshot(_)
                            | protocol_envelope::Body::DescriptorDelta(_)
                    ) =>
                {
                    topology_faults(instance, activation_epoch, sink);
                    health.update(instance.health_entry(
                        RuntimeHealthStatus::Degraded,
                        Some("descriptor changed during active plan".into()),
                    ));
                }
                Ok(None) => {}
                Err(error) => health.update(
                    instance.health_entry(RuntimeHealthStatus::Degraded, Some(error.to_string())),
                ),
            }
        }
        Err(ProcessError::Timeout) => return false,
        Err(_) => {
            let now = elapsed_millis(clock);
            if let Ok(mut registry) = registry.lock() {
                let _ignored = instance
                    .controller
                    .process_exited(&mut registry, now, false);
            }
            health.update(instance.health_entry(
                RuntimeHealthStatus::Unavailable,
                Some("plugin process exited".into()),
            ));
            topology_faults(instance, activation_epoch, sink);
            return true;
        }
    }
    false
}

fn process_stopped(
    instance: &mut RuntimeInstance,
    registry: &Arc<Mutex<CapabilityRegistry>>,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
    health: &RuntimeHealthStore,
    clock: Instant,
) -> bool {
    let now = elapsed_millis(clock);
    if instance.grants_changed().unwrap_or(true) {
        let action = registry
            .lock()
            .ok()
            .and_then(|mut registry| instance.revalidate_permissions(&mut registry, now).ok());
        if !matches!(action, Some(SupervisorAction::None)) {
            sink.publish(RuntimeEvent {
                activation_epoch,
                kind: RuntimeEventKind::GrantFault,
            });
            health.update(instance.health_entry(
                RuntimeHealthStatus::Unavailable,
                Some("runtime permission revoked".into()),
            ));
            stop(instance, registry, now);
            return true;
        }
    }
    match instance.controller.tick(now) {
        Ok(SupervisorAction::ForceTerminate | SupervisorAction::RequestGracefulStop) => {
            stop(instance, registry, now);
            true
        }
        Ok(_) => false,
        Err(_) => true,
    }
}

fn stop(instance: &mut RuntimeInstance, registry: &Arc<Mutex<CapabilityRegistry>>, now: u64) {
    if let Ok(mut registry) = registry.lock() {
        let request = envelope(
            instance.generation,
            &mut instance.next_message_id,
            protocol_envelope::Body::Deactivate(deckweave_protocol::v1::DeactivationRequest {
                runtime_lease_id: instance.lease_id.to_string(),
                reason: "runtime monitor stop".into(),
            }),
        );
        if let Ok(request) = request {
            let _ignored = instance.process.send(&request);
        }
        let _ignored = instance.process.terminate();
        let _ignored = instance.controller.process_exited(&mut registry, now, true);
    }
}

fn topology_faults(
    instance: &RuntimeInstance,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
) {
    for endpoint in instance.stable_endpoints() {
        sink.publish(RuntimeEvent {
            activation_epoch,
            kind: RuntimeEventKind::TopologyFault { endpoint },
        });
    }
}

fn dispatch_command(
    instance: &mut RuntimeInstance,
    command: &RuntimeCommand,
) -> Result<(), HostRuntimeError> {
    let Some(body) = wire::command_body(command)? else {
        return Ok(());
    };
    let request = envelope(instance.generation, &mut instance.next_message_id, body)?;
    instance.process.send(&request)?;
    Ok(())
}
