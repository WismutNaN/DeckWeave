// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime topology and canonical physical-resource ownership.

use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        DescriptorRevision, NodeId, PackageId, PluginSessionId, PortId, RegistryRevision,
        ResourceLeaseId, RuntimeGroupId, SessionGeneration,
    },
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
mod descriptors;
use descriptors::{find_port, validate_node_set, validate_nodes};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentity {
    pub package_id: PackageId,
    pub runtime_group_id: RuntimeGroupId,
    pub plugin_session_id: PluginSessionId,
    pub generation: SessionGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StablePortRef {
    pub package_id: PackageId,
    pub runtime_group_id: RuntimeGroupId,
    pub node_id: NodeId,
    pub port_id: PortId,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CanonicalResourceKey(String);

impl CanonicalResourceKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, RegistryError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 256
            || value.chars().any(|character| character.is_control())
        {
            return Err(RegistryError::InvalidResourceKey);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CanonicalResourceKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLeaseMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLeaseRecord {
    pub lease_id: ResourceLeaseId,
    pub owner_package_id: PackageId,
    pub resource: CanonicalResourceKey,
    pub mode: ResourceLeaseMode,
    pub purpose: String,
}

#[derive(Debug, Clone)]
struct SessionState {
    identity: SessionIdentity,
    descriptor_revision: DescriptorRevision,
    snapshot_registered: bool,
    nodes: BTreeMap<NodeId, NodeDescriptor>,
}

#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    sessions: BTreeMap<(PackageId, RuntimeGroupId), SessionState>,
    resources: BTreeMap<ResourceLeaseId, ResourceLeaseRecord>,
    revision: u64,
    next_resource_lease: u64,
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
    pub fn start_session(&mut self, identity: SessionIdentity) -> Result<(), RegistryError> {
        let key = (
            identity.package_id.clone(),
            identity.runtime_group_id.clone(),
        );
        if self
            .sessions
            .get(&key)
            .is_some_and(|current| current.identity.generation >= identity.generation)
        {
            return Err(RegistryError::StaleSession);
        }
        self.sessions.insert(
            key,
            SessionState {
                identity,
                descriptor_revision: DescriptorRevision::try_new(1)
                    .map_err(|_| RegistryError::CounterExhausted)?,
                nodes: BTreeMap::new(),
                snapshot_registered: false,
            },
        );
        self.bump_revision()?;
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
        if state.snapshot_registered && descriptor_revision == state.descriptor_revision {
            return if state.nodes == validated {
                Ok(())
            } else {
                Err(RegistryError::StaleDescriptorRevision)
            };
        }
        state.descriptor_revision = descriptor_revision;
        state.nodes = validated;
        state.snapshot_registered = true;
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
        if !state.snapshot_registered || state.descriptor_revision != base_revision {
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
        self.bump_revision()?;
        Ok(())
    }

    pub fn resolve(&self, reference: &StablePortRef) -> Result<ResolvedEndpoint, RegistryError> {
        let state = self
            .sessions
            .get(&(
                reference.package_id.clone(),
                reference.runtime_group_id.clone(),
            ))
            .ok_or(RegistryError::UnknownSession)?;
        let node = state
            .nodes
            .get(&reference.node_id)
            .ok_or(RegistryError::PortUnavailable)?;
        if find_port(node, &reference.port_id).is_none() {
            return Err(RegistryError::PortUnavailable);
        }
        Ok(ResolvedEndpoint {
            reference: reference.clone(),
            session: state.identity.clone(),
            descriptor_revision: state.descriptor_revision,
        })
    }

    pub fn acquire_resource(
        &mut self,
        owner_package_id: PackageId,
        resource: CanonicalResourceKey,
        mode: ResourceLeaseMode,
        purpose: impl Into<String>,
    ) -> Result<ResourceLeaseRecord, RegistryError> {
        let conflict = self.resources.values().any(|existing| {
            existing.resource == resource
                && (existing.mode == ResourceLeaseMode::Exclusive
                    || mode == ResourceLeaseMode::Exclusive)
        });
        if conflict {
            return Err(RegistryError::ResourceConflict);
        }
        self.next_resource_lease = self
            .next_resource_lease
            .checked_add(1)
            .ok_or(RegistryError::CounterExhausted)?;
        let lease = ResourceLeaseRecord {
            lease_id: ResourceLeaseId::try_new(format!(
                "resource-lease-{}",
                self.next_resource_lease
            ))
            .map_err(|_| RegistryError::CounterExhausted)?,
            owner_package_id,
            resource,
            mode,
            purpose: purpose.into(),
        };
        self.resources.insert(lease.lease_id.clone(), lease.clone());
        self.bump_revision()?;
        Ok(lease)
    }

    pub fn release_resource(&mut self, lease_id: &ResourceLeaseId) -> Result<(), RegistryError> {
        self.resources
            .remove(lease_id)
            .ok_or(RegistryError::UnknownResourceLease)?;
        self.bump_revision()?;
        Ok(())
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
            resource_leases: self.resources.values().cloned().collect(),
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
