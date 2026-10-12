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

mod binding;
mod wire;
mod worker;
pub(super) use worker::{ActiveWorker, WorkerCommand, WorkerKey};

pub(super) struct RuntimeInstance {
    controller: RuntimeController,
    process: ProcessRuntime,
    lease_id: RuntimeLeaseId,
    contribution_id: ContributionId,
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

    fn stable_endpoints(&self) -> Vec<deckweave_domain::endpoint::StableEndpointRef> {
        self.nodes
            .iter()
            .flat_map(|node| {
                let package_id = self.controller.health().key.package_id;
                let node_id = node.node_id.clone();
                node.devices
                    .iter()
                    .flat_map({
                        let package_id = package_id.clone();
                        let node_id = node_id.clone();
                        move |device| {
                            let target = deckweave_domain::endpoint::EndpointTargetRef::Device {
                                provider_id: device.provider_id.clone(),
                                device_identity: device.identity.clone(),
                            };
                            device.elements.iter().flat_map({
                                let package_id = package_id.clone();
                                let node_id = node_id.clone();
                                move |element| {
                                    element.capabilities.iter().flat_map({
                                        let package_id = package_id.clone();
                                        let node_id = node_id.clone();
                                        let target = target.clone();
                                        move |capability| {
                                            capability.ports.iter().map({
                                                let package_id = package_id.clone();
                                                let node_id = node_id.clone();
                                                let target = target.clone();
                                                let element_id = element.element_id.clone();
                                                let capability_id =
                                                    capability.capability_id.clone();
                                                move |port| {
                                                    deckweave_domain::endpoint::StableEndpointRef {
                                                        package_id: package_id.clone(),
                                                        node_id: node_id.clone(),
                                                        target: target.clone(),
                                                        port:
                                                            deckweave_domain::endpoint::PortPath {
                                                                element_id: element_id.clone(),
                                                                capability_id: capability_id
                                                                    .clone(),
                                                                port_id: port.port_id.clone(),
                                                            },
                                                    }
                                                }
                                            })
                                        }
                                    })
                                }
                            })
                        }
                    })
                    .chain(node.services.iter().flat_map({
                        let package_id = package_id.clone();
                        let node_id = node_id.clone();
                        move |service| {
                            let target = deckweave_domain::endpoint::EndpointTargetRef::Service {
                                provider_id: service.provider_id.clone(),
                                service_id: service.service_id.clone(),
                            };
                            service.elements.iter().flat_map({
                                let package_id = package_id.clone();
                                let node_id = node_id.clone();
                                move |element| {
                                    element.capabilities.iter().flat_map({
                                        let package_id = package_id.clone();
                                        let node_id = node_id.clone();
                                        let target = target.clone();
                                        move |capability| {
                                            capability.ports.iter().map({
                                                let package_id = package_id.clone();
                                                let node_id = node_id.clone();
                                                let target = target.clone();
                                                let element_id = element.element_id.clone();
                                                let capability_id =
                                                    capability.capability_id.clone();
                                                move |port| {
                                                    deckweave_domain::endpoint::StableEndpointRef {
                                                        package_id: package_id.clone(),
                                                        node_id: node_id.clone(),
                                                        target: target.clone(),
                                                        port:
                                                            deckweave_domain::endpoint::PortPath {
                                                                element_id: element_id.clone(),
                                                                capability_id: capability_id
                                                                    .clone(),
                                                                port_id: port.port_id.clone(),
                                                            },
                                                    }
                                                }
                                            })
                                        }
                                    })
                                }
                            })
                        }
                    }))
            })
            .collect()
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
