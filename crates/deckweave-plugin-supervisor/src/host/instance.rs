// SPDX-License-Identifier: GPL-3.0-or-later
//! One isolated runtime instance from launch through bounded shutdown.

use super::config::{HostRuntimeConfig, HostRuntimeError};
use crate::{
    package::IndexedPackage,
    process::{ProcessError, ProcessRuntime},
    supervisor::{RuntimeController, RuntimeLeaseReason, SupervisorAction},
};
use deckweave_application::AuthorizationStore;
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        ContributionId, DescriptorRevision, GrantRevision, PluginSessionId, PrincipalId,
        RuntimeGroupId, RuntimeLeaseId,
    },
};
use deckweave_platform::ChildProcessPolicy;
use deckweave_protocol::v1::{
    ActivationRequest, DeactivationRequest, ProtocolEnvelope, ProtocolVersion, protocol_envelope,
};
use deckweave_registry::CapabilityRegistry;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

pub(super) struct RuntimeInstance {
    controller: RuntimeController,
    process: ProcessRuntime,
    lease_id: RuntimeLeaseId,
    descriptor_revision: DescriptorRevision,
    nodes: Vec<NodeDescriptor>,
    generation: u64,
    next_message_id: u64,
    authorization: Arc<dyn AuthorizationStore>,
    principal: PrincipalId,
    grant_revision: GrantRevision,
}

impl RuntimeInstance {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn launch(
        package: Arc<IndexedPackage>,
        runtime_group: RuntimeGroupId,
        contribution: ContributionId,
        generation: u64,
        config: &HostRuntimeConfig,
        authorization: Arc<dyn AuthorizationStore>,
        principal: PrincipalId,
        process_policy: &dyn ChildProcessPolicy,
        now: u64,
    ) -> Result<Self, HostRuntimeError> {
        let mut controller = RuntimeController::new_after_generation(
            Arc::clone(&package),
            runtime_group.clone(),
            config.restart_policy.clone(),
            generation.saturating_sub(1),
        )?;
        let (lease, action) = controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
        if action != SupervisorAction::CheckPermissions
            || controller.check_permissions(&principal, authorization.as_ref())?
                != SupervisorAction::StartProcess
        {
            return Err(HostRuntimeError::Protocol(
                "runtime did not enter process start state".into(),
            ));
        }
        let token = ProcessRuntime::random_token()?;
        let mut process = ProcessRuntime::spawn_indexed(
            &package,
            &runtime_group,
            &token,
            generation,
            config.process.clone(),
            &BTreeMap::new(),
            process_policy,
        )?;
        controller.process_started(
            PluginSessionId::try_new(format!("session-{generation}"))
                .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?,
            token,
            now,
        )?;
        let hello = process.receive(config.handshake_timeout)?;
        let Some(protocol_envelope::Body::Hello(hello)) = hello.body else {
            return Err(HostRuntimeError::Protocol("expected plugin hello".into()));
        };
        let ack = controller.accept_hello(&hello, &config.protocol_support)?;
        let maximum = usize::try_from(
            ack.limits
                .as_ref()
                .ok_or_else(|| HostRuntimeError::Protocol("hello ack has no limits".into()))?
                .maximum_message_bytes,
        )
        .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
        process.constrain_maximum_message_bytes(maximum)?;
        let mut next_message_id = 0;
        process.send(&envelope(
            generation,
            &mut next_message_id,
            protocol_envelope::Body::HelloAck(ack),
        )?)?;
        process.send(&envelope(
            generation,
            &mut next_message_id,
            protocol_envelope::Body::Activate(ActivationRequest {
                runtime_lease_id: lease.lease_id.to_string(),
                contribution_id: contribution.to_string(),
                settings: None,
            }),
        )?)?;
        let activated = process.receive(config.handshake_timeout)?;
        let Some(protocol_envelope::Body::Activated(activated)) = activated.body else {
            return Err(HostRuntimeError::Protocol(
                "expected plugin activation reply".into(),
            ));
        };
        if activated.runtime_lease_id != lease.lease_id.as_str() {
            return Err(HostRuntimeError::Protocol(
                "activation lease identity mismatch".into(),
            ));
        }
        let descriptor = activated
            .descriptor
            .ok_or_else(|| HostRuntimeError::Protocol("activation has no descriptor".into()))?;
        let nodes = serde_json::from_slice(&descriptor.value)
            .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
        let descriptor_revision = DescriptorRevision::try_new(activated.descriptor_revision)
            .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
        let grant_revision = authorization.revision()?;
        Ok(Self {
            controller,
            process,
            lease_id: lease.lease_id,
            descriptor_revision,
            nodes,
            generation,
            next_message_id,
            authorization,
            principal,
            grant_revision,
        })
    }

    pub(super) fn publish(
        &mut self,
        registry: &mut CapabilityRegistry,
        now: u64,
    ) -> Result<(), HostRuntimeError> {
        self.controller.activate_descriptors(
            registry,
            self.descriptor_revision,
            self.nodes.clone(),
            now,
        )?;
        Ok(())
    }

    pub(super) fn stop(
        mut self,
        registry: &mut CapabilityRegistry,
        timeout: Duration,
        now: u64,
    ) -> Result<(), HostRuntimeError> {
        let request = envelope(
            self.generation,
            &mut self.next_message_id,
            protocol_envelope::Body::Deactivate(DeactivationRequest {
                runtime_lease_id: self.lease_id.to_string(),
                reason: "host shutdown".into(),
            }),
        )?;
        if self.process.send(&request).is_ok() {
            if self.process.wait_timeout(timeout)?.is_none() {
                self.process.terminate()?;
            }
        } else {
            self.process.terminate()?;
        }
        self.controller.process_exited(registry, now, true)?;
        Ok(())
    }

    fn grants_changed(&self) -> Result<bool, HostRuntimeError> {
        Ok(self.authorization.revision()? != self.grant_revision)
    }

    fn revalidate_permissions(
        &mut self,
        registry: &mut CapabilityRegistry,
        now: u64,
    ) -> Result<SupervisorAction, HostRuntimeError> {
        let revision = self.authorization.revision()?;
        let action = self.controller.revalidate_permissions(
            &self.principal,
            self.authorization.as_ref(),
            registry,
            now,
        )?;
        self.grant_revision = revision;
        Ok(action)
    }
}

pub(super) struct ActiveWorker {
    shutdown: mpsc::SyncSender<mpsc::SyncSender<()>>,
    join: Option<thread::JoinHandle<()>>,
}

impl ActiveWorker {
    pub(super) fn spawn(
        instance: RuntimeInstance,
        registry: Arc<Mutex<CapabilityRegistry>>,
        config: HostRuntimeConfig,
        clock: Instant,
    ) -> Self {
        let (shutdown, receiver) = mpsc::sync_channel(1);
        let join = thread::spawn(move || {
            monitor(instance, registry, config, clock, receiver);
        });
        Self {
            shutdown,
            join: Some(join),
        }
    }

    pub(super) fn shutdown(mut self, timeout: Duration) -> Result<(), HostRuntimeError> {
        let (done, receiver) = mpsc::sync_channel(1);
        match self.shutdown.try_send(done) {
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
}

impl Drop for ActiveWorker {
    fn drop(&mut self) {
        let (done, _receiver) = mpsc::sync_channel(1);
        let _ignored = self.shutdown.try_send(done);
    }
}

fn monitor(
    mut instance: RuntimeInstance,
    registry: Arc<Mutex<CapabilityRegistry>>,
    config: HostRuntimeConfig,
    clock: Instant,
    shutdown: mpsc::Receiver<mpsc::SyncSender<()>>,
) {
    loop {
        if let Ok(done) = shutdown.try_recv() {
            let now = elapsed_millis(clock);
            let result = registry.lock().ok().and_then(|mut registry| {
                instance.stop(&mut registry, config.stop_timeout, now).ok()
            });
            let _ignored = done.try_send(());
            let _ = result;
            return;
        }
        match instance.process.receive(config.monitor_interval) {
            Ok(envelope) => {
                let now = elapsed_millis(clock);
                let _ignored = instance.controller.accept_envelope(&envelope, now);
            }
            Err(ProcessError::Timeout) => {}
            Err(_) => {
                let now = elapsed_millis(clock);
                if let Ok(mut registry) = registry.lock() {
                    let _ignored = instance
                        .controller
                        .process_exited(&mut registry, now, false);
                }
                return;
            }
        }
        let now = elapsed_millis(clock);
        if instance.grants_changed().unwrap_or(true) {
            let action = registry
                .lock()
                .ok()
                .and_then(|mut registry| instance.revalidate_permissions(&mut registry, now).ok());
            if !matches!(action, Some(SupervisorAction::None)) {
                if let Ok(mut registry) = registry.lock() {
                    let _ignored = instance.stop(&mut registry, config.stop_timeout, now);
                }
                return;
            }
        }
        match instance.controller.tick(now) {
            Ok(SupervisorAction::ForceTerminate | SupervisorAction::RequestGracefulStop) => {
                if let Ok(mut registry) = registry.lock() {
                    let _ignored = instance.stop(&mut registry, config.stop_timeout, now);
                }
                return;
            }
            Ok(_) => {}
            Err(_) => return,
        }
    }
}

fn envelope(
    generation: u64,
    message_id: &mut u64,
    body: protocol_envelope::Body,
) -> Result<ProtocolEnvelope, HostRuntimeError> {
    *message_id = message_id
        .checked_add(1)
        .ok_or_else(|| HostRuntimeError::Protocol("message ID exhausted".into()))?;
    Ok(ProtocolEnvelope {
        wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
        session_generation: generation,
        message_id: *message_id,
        trace: None,
        body: Some(body),
    })
}

pub(super) fn elapsed_millis(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}
