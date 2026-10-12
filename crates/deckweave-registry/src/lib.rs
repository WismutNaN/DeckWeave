// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime topology and canonical physical-resource ownership.

pub use deckweave_domain::endpoint::{EndpointTargetRef, PortPath, StableEndpointRef};
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        DescriptorRevision, NodeId, PackageId, PluginSessionId, RegistryRevision, ResourceLeaseId,
        RuntimeGroupId, SessionGeneration,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod descriptors;
use descriptors::{find_port, validate_node_set, validate_nodes};
mod resources;
use resources::ResourceArbiter;
pub use resources::{
    CanonicalResourceKey, ResourceLeaseMode, ResourceLeaseOwner, ResourceLeaseRecord,
    ResourceOperationKind,
};
use thiserror::Error;

/// Compatibility alias for early registry consumers. New persisted contracts
/// should use the domain-owned `StableEndpointRef` name.
pub type StablePortRef = StableEndpointRef;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentity {
    pub package_id: PackageId,
    pub runtime_group_id: RuntimeGroupId,
    pub plugin_session_id: PluginSessionId,
    pub generation: SessionGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedEndpoint {
    pub reference: StablePortRef,
    pub session: SessionIdentity,
    pub descriptor_revision: DescriptorRevision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredNode {
    pub session: SessionIdentity,
    pub descriptor: NodeDescriptor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySnapshot {
    pub revision: RegistryRevision,
    pub nodes: Vec<RegisteredNode>,
    pub resource_leases: Vec<ResourceLeaseRecord>,
}

#[derive(Debug, Clone)]
struct SessionState {
    identity: SessionIdentity,
    descriptor_revision: DescriptorRevision,
    nodes: BTreeMap<NodeId, NodeDescriptor>,
}

#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    sessions: BTreeMap<(PackageId, RuntimeGroupId), SessionState>,
    resources: ResourceArbiter,
    revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error("session generation должна расти монотонно")]
    StaleSession,
    #[error("session не зарегистрирована")]
    UnknownSession,
    #[error("delta base revision не совпадает; требуется полный snapshot")]
    ResyncRequired,
    #[error("descriptor revision должна расти")]
    StaleDescriptorRevision,
    #[error("повторяющийся node ID: {0}")]
    DuplicateNode(String),
    #[error("descriptor не прошёл validation: {0}")]
    InvalidDescriptor(String),
    #[error("порт не найден или удалён")]
    PortUnavailable,
    #[error("stable endpoint опубликован несколькими runtime groups")]
    AmbiguousEndpoint,
    #[error("canonical resource key пуст, слишком длинен или содержит control characters")]
    InvalidResourceKey,
    #[error("resource уже принадлежит конфликтующему owner")]
    ResourceConflict,
    #[error("resource lease не найдена")]
    UnknownResourceLease,
    #[error("внутренний counter исчерпан")]
    CounterExhausted,
}

impl CapabilityRegistry {
    pub fn register_initial_snapshot(
        &mut self,
        identity: SessionIdentity,
        descriptor_revision: DescriptorRevision,
        nodes: Vec<NodeDescriptor>,
    ) -> Result<(), RegistryError> {
        let validated = validate_nodes(nodes, descriptor_revision)?;
        let key = (
            identity.package_id.clone(),
            identity.runtime_group_id.clone(),
        );
        if let Some(current) = self.sessions.get(&key) {
            if current.identity == identity
                && current.descriptor_revision == descriptor_revision
                && current.nodes == validated
            {
                return Ok(());
            }
            if current.identity.generation >= identity.generation {
                return Err(RegistryError::StaleSession);
            }
        }

        let next_registry_revision = self
            .revision
            .checked_add(1)
            .ok_or(RegistryError::CounterExhausted)?;
        let replaced = self.sessions.insert(
            key,
            SessionState {
                identity,
                descriptor_revision,
                nodes: validated,
            },
        );
        if let Some(replaced) = replaced {
            self.resources
                .release_owner(&ResourceLeaseOwner::RuntimeSession {
                    session: replaced.identity,
                });
        }
        self.revision = next_registry_revision;
        Ok(())
    }

    pub fn register_snapshot(
        &mut self,
        session: &SessionIdentity,
        descriptor_revision: DescriptorRevision,
        nodes: Vec<NodeDescriptor>,
    ) -> Result<(), RegistryError> {
        let validated = validate_nodes(nodes, descriptor_revision)?;
        let state = self.current_session_mut(session)?;
        if descriptor_revision < state.descriptor_revision {
            return Err(RegistryError::StaleDescriptorRevision);
        }
        if descriptor_revision == state.descriptor_revision {
            return if state.nodes == validated {
                Ok(())
            } else {
                Err(RegistryError::StaleDescriptorRevision)
            };
        }
        state.descriptor_revision = descriptor_revision;
        state.nodes = validated;
        self.bump_revision()?;
        Ok(())
    }

    pub fn apply_delta(
        &mut self,
        session: &SessionIdentity,
        base_revision: DescriptorRevision,
        descriptor_revision: DescriptorRevision,
        upserts: Vec<NodeDescriptor>,
        removed_node_ids: &[NodeId],
    ) -> Result<(), RegistryError> {
        let state = self.current_session(session)?;
        if state.descriptor_revision != base_revision {
            return Err(RegistryError::ResyncRequired);
        }
        if descriptor_revision <= base_revision {
            return Err(RegistryError::StaleDescriptorRevision);
        }

        let mut candidate = state.nodes.clone();
        for removed in removed_node_ids {
            candidate.remove(removed);
        }
        let upserts = validate_nodes(upserts, descriptor_revision)?;
        candidate.extend(upserts);
        validate_node_set(&candidate)?;

        let state = self.current_session_mut(session)?;
        state.nodes = candidate;
        state.descriptor_revision = descriptor_revision;
        self.bump_revision()?;
        Ok(())
    }

    pub fn invalidate_session(&mut self, session: &SessionIdentity) -> Result<(), RegistryError> {
        let key = (session.package_id.clone(), session.runtime_group_id.clone());
        if !self
            .sessions
            .get(&key)
            .is_some_and(|current| current.identity == *session)
        {
            return Err(RegistryError::StaleSession);
        }
        self.sessions.remove(&key);
        self.resources
            .release_owner(&ResourceLeaseOwner::RuntimeSession {
                session: session.clone(),
            });
        self.bump_revision()?;
        Ok(())
    }

    pub fn resolve(&self, reference: &StablePortRef) -> Result<ResolvedEndpoint, RegistryError> {
        let mut matches = self
            .sessions
            .values()
            .filter(|state| state.identity.package_id == reference.package_id)
            .filter(|state| {
                state.nodes.get(&reference.node_id).is_some_and(|node| {
                    find_port(node, &reference.target, &reference.port).is_some()
                })
            });
        let state = matches.next().ok_or(RegistryError::PortUnavailable)?;
        if matches.next().is_some() {
            return Err(RegistryError::AmbiguousEndpoint);
        }
        Ok(ResolvedEndpoint {
            reference: reference.clone(),
            session: state.identity.clone(),
            descriptor_revision: state.descriptor_revision,
        })
    }

    pub fn acquire_resource(
        &mut self,
        owner: ResourceLeaseOwner,
        resource: CanonicalResourceKey,
        mode: ResourceLeaseMode,
        purpose: impl Into<String>,
    ) -> Result<ResourceLeaseRecord, RegistryError> {
        if let ResourceLeaseOwner::RuntimeSession { session } = &owner {
            self.current_session(session)?;
        }
        let lease = self
            .resources
            .acquire(owner, resource, mode, purpose.into())?;
        self.bump_revision()?;
        Ok(lease)
    }

    pub fn release_resource(&mut self, lease_id: &ResourceLeaseId) -> Result<(), RegistryError> {
        self.resources.release(lease_id)?;
        self.bump_revision()?;
        Ok(())
    }

    pub fn release_resources_for_owner(
        &mut self,
        owner: &ResourceLeaseOwner,
    ) -> Result<usize, RegistryError> {
        let released = self.resources.release_owner(owner);
        if released > 0 {
            self.bump_revision()?;
        }
        Ok(released)
    }

    pub fn expire_resource_operations(
        &mut self,
        now_host_ticks: u64,
    ) -> Result<usize, RegistryError> {
        let released = self.resources.expire_operations(now_host_ticks);
        if released > 0 {
            self.bump_revision()?;
        }
        Ok(released)
    }

    pub fn snapshot(&self) -> Result<RegistrySnapshot, RegistryError> {
        let revision = RegistryRevision::try_new(self.revision.max(1))
            .map_err(|_| RegistryError::CounterExhausted)?;
        Ok(RegistrySnapshot {
            revision,
            nodes: self
                .sessions
                .values()
                .flat_map(|state| {
                    state
                        .nodes
                        .values()
                        .cloned()
                        .map(|descriptor| RegisteredNode {
                            session: state.identity.clone(),
                            descriptor,
                        })
                })
                .collect(),
            resource_leases: self.resources.records().cloned().collect(),
        })
    }

    fn current_session(&self, session: &SessionIdentity) -> Result<&SessionState, RegistryError> {
        self.sessions
            .get(&(session.package_id.clone(), session.runtime_group_id.clone()))
            .filter(|state| state.identity == *session)
            .ok_or(RegistryError::StaleSession)
    }

    fn current_session_mut(
        &mut self,
        session: &SessionIdentity,
    ) -> Result<&mut SessionState, RegistryError> {
        self.sessions
            .get_mut(&(session.package_id.clone(), session.runtime_group_id.clone()))
            .filter(|state| state.identity == *session)
            .ok_or(RegistryError::StaleSession)
    }

    fn bump_revision(&mut self) -> Result<(), RegistryError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(RegistryError::CounterExhausted)?;
        Ok(())
    }
}
