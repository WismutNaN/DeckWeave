// SPDX-License-Identifier: GPL-3.0-or-later
//! One isolated runtime instance from launch through bounded shutdown.

use super::config::{HostRuntimeConfig, HostRuntimeError};
use super::health::{RuntimeHealthEntry, RuntimeHealthStatus};
use crate::{
    package::IndexedPackage,
    process::ProcessRuntime,
    supervisor::{RuntimeController, RuntimeLeaseReason, SupervisorAction},
};
use deckweave_application::AuthorizationStore;
use deckweave_automation::{CompiledEndpointContract, RuntimeBinding};
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
    sync::Arc,
    time::{Duration, Instant},
};

mod activation;
mod binding;
mod endpoints;
mod wire;
mod worker;
pub(super) use worker::{ActiveWorker, WorkerCommand, WorkerKey};

pub(super) struct RuntimeInstance {
    replies: wire::ReplyContracts,
    cleanups: BTreeMap<String, Instant>,
    cleanup_failed: bool,
    failed_release: Arc<std::sync::atomic::AtomicBool>,
    activation_nodes: Vec<(
        ContributionId,
        Option<deckweave_domain::ids::PluginInstanceId>,
        Vec<NodeDescriptor>,
    )>,
    controller: RuntimeController,
    process: ProcessRuntime,
    contribution_id: ContributionId,
    contributions: Vec<ContributionId>,
    activation_leases: Vec<RuntimeLeaseId>,
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
        configuration: Option<&deckweave_workspace::PluginInstanceConfiguration>,
        generation: u64,
        config: &HostRuntimeConfig,
        authorization: Arc<dyn AuthorizationStore>,
        principal: PrincipalId,
        process_policy: &dyn ChildProcessPolicy,
        now: u64,
    ) -> Result<Self, HostRuntimeError> {
        config
            .locale
            .validate()
            .map_err(|e| HostRuntimeError::Protocol(e.into()))?;
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
        activation::validate_features(&controller, configuration)?;
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
                settings: activation::settings_payload(configuration)?,
                locale: Some(wire::locale_context(&config.locale)),
                configured_instance_id: configuration.map(|c| c.instance_id.to_string()),
                settings_revision: configuration.map_or(0, |c| c.settings_revision),
                credential_refs: configuration
                    .map(|c| {
                        c.credentials
                            .iter()
                            .map(|(key, value)| (key.clone(), value.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
        )?)?;
        let activated = process.receive(config.handshake_timeout)?;
        controller.accept_envelope(&activated, now)?;
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
        let nodes = activation::decode_nodes(&descriptor)?;
        let descriptor_revision = DescriptorRevision::try_new(activated.descriptor_revision)
            .map_err(|error| HostRuntimeError::Protocol(error.to_string()))?;
        let grant_revision = authorization.revision()?;
        Ok(Self {
            replies: wire::ReplyContracts::default(),
            cleanups: BTreeMap::new(),
            cleanup_failed: false,
            failed_release: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            activation_nodes: vec![(
                contribution.clone(),
                configuration.map(|c| c.instance_id.clone()),
                nodes.clone(),
            )],
            controller,
            process,
            activation_leases: vec![lease.lease_id.clone()],
            contributions: vec![contribution.clone()],
            contribution_id: contribution,
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

    pub(super) fn ensure_alive(&mut self) -> Result<(), HostRuntimeError> {
        if self.process.try_wait()?.is_some() {
            return Err(HostRuntimeError::Protocol(
                "prepared runtime exited before the commit barrier".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn bindings(
        &self,
        contracts: &[CompiledEndpointContract],
    ) -> Result<Vec<RuntimeBinding>, HostRuntimeError> {
        binding::runtime_bindings(self, contracts)
    }

    pub(super) fn stop(
        mut self,
        registry: &mut CapabilityRegistry,
        timeout: Duration,
        now: u64,
    ) -> Result<(), HostRuntimeError> {
        let mut sent = true;
        for lease in &self.activation_leases {
            let request = envelope(
                self.generation,
                &mut self.next_message_id,
                protocol_envelope::Body::Deactivate(DeactivationRequest {
                    runtime_lease_id: lease.to_string(),
                    reason: "host shutdown".into(),
                }),
            )?;
            sent &= self.process.send(&request).is_ok();
        }
        if sent {
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

    fn worker_identity(&self) -> Result<(WorkerKey, BTreeMap<String, u32>), HostRuntimeError> {
        let session = self
            .controller
            .session()
            .ok_or_else(|| HostRuntimeError::Protocol("runtime session is missing".into()))?;
        Ok((
            WorkerKey {
                package_id: session.identity.package_id.clone(),
                runtime_group_id: session.identity.runtime_group_id.clone(),
                plugin_session_id: session.identity.plugin_session_id.clone(),
                generation: session.identity.generation,
            },
            session.negotiated_features.clone(),
        ))
    }

    fn health_entry(
        &self,
        status: RuntimeHealthStatus,
        detail: Option<String>,
    ) -> RuntimeHealthEntry {
        let health = self.controller.health();
        RuntimeHealthEntry {
            package_id: health.key.package_id,
            runtime_group_id: health.key.runtime_group_id,
            contribution_id: self.contribution_id.clone(),
            generation: self.generation,
            status,
            detail,
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
