// SPDX-License-Identifier: GPL-3.0-or-later
//! Canonical physical-resource arbitration and owner-scoped cleanup.

use super::{RegistryError, SessionIdentity};
use deckweave_domain::ids::{OperationId, PackageId, ResourceLeaseId};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceOperationKind {
    Prepare,
    Probe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ResourceLeaseOwner {
    RuntimeSession {
        session: SessionIdentity,
    },
    Operation {
        package_id: PackageId,
        operation_id: OperationId,
        kind: ResourceOperationKind,
        expires_at_host_ticks: u64,
    },
}

impl ResourceLeaseOwner {
    #[must_use]
    pub fn package_id(&self) -> &PackageId {
        match self {
            Self::RuntimeSession { session } => &session.package_id,
            Self::Operation { package_id, .. } => package_id,
        }
    }

    fn is_expired_operation(&self, now_host_ticks: u64) -> bool {
        matches!(
            self,
            Self::Operation {
                expires_at_host_ticks,
                ..
            } if *expires_at_host_ticks <= now_host_ticks
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLeaseRecord {
    pub lease_id: ResourceLeaseId,
    pub owner: ResourceLeaseOwner,
    pub resource: CanonicalResourceKey,
    pub mode: ResourceLeaseMode,
    pub purpose: String,
}

#[derive(Debug, Default)]
pub(super) struct ResourceArbiter {
    leases: BTreeMap<ResourceLeaseId, ResourceLeaseRecord>,
    next_lease: u64,
}

impl ResourceArbiter {
    pub(super) fn acquire(
        &mut self,
        owner: ResourceLeaseOwner,
        resource: CanonicalResourceKey,
        mode: ResourceLeaseMode,
        purpose: String,
    ) -> Result<ResourceLeaseRecord, RegistryError> {
        let conflict = self.leases.values().any(|existing| {
            existing.resource == resource
                && (existing.mode == ResourceLeaseMode::Exclusive
                    || mode == ResourceLeaseMode::Exclusive)
        });
        if conflict {
            return Err(RegistryError::ResourceConflict);
        }

        self.next_lease = self
            .next_lease
            .checked_add(1)
            .ok_or(RegistryError::CounterExhausted)?;
        let lease = ResourceLeaseRecord {
            lease_id: ResourceLeaseId::try_new(format!("resource-lease-{}", self.next_lease))
                .map_err(|_| RegistryError::CounterExhausted)?,
            owner,
            resource,
            mode,
            purpose,
        };
        self.leases.insert(lease.lease_id.clone(), lease.clone());
        Ok(lease)
    }

    pub(super) fn release(&mut self, lease_id: &ResourceLeaseId) -> Result<(), RegistryError> {
        self.leases
            .remove(lease_id)
            .ok_or(RegistryError::UnknownResourceLease)?;
        Ok(())
    }

    pub(super) fn release_owner(&mut self, owner: &ResourceLeaseOwner) -> usize {
        let before = self.leases.len();
        self.leases.retain(|_, lease| lease.owner != *owner);
        before - self.leases.len()
    }

    pub(super) fn expire_operations(&mut self, now_host_ticks: u64) -> usize {
        let before = self.leases.len();
        self.leases
            .retain(|_, lease| !lease.owner.is_expired_operation(now_host_ticks));
        before - self.leases.len()
    }

    pub(super) fn records(&self) -> impl Iterator<Item = &ResourceLeaseRecord> {
        self.leases.values()
    }
}
