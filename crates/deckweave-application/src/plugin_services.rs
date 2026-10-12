// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-issued service authority and bounded nested calls. No direct plugin sockets.
use deckweave_domain::{
    ids::{
        ActivationEpoch, ExecutionId, GrantRevision, NamespacedKind, PluginInstanceId,
        SessionGeneration,
    },
    schema::SchemaRef,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceOwner {
    pub execution_id: ExecutionId,
    pub activation_epoch: ActivationEpoch,
    pub caller_instance: PluginInstanceId,
    pub caller_generation: SessionGeneration,
    pub grant_revision: GrantRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationContext {
    pub owner: ServiceOwner,
    pub deadline_host_ticks: u64,
    pub call_path: Vec<PluginInstanceId>,
    pub allowed_operations: BTreeSet<NamespacedKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ServiceCallError {
    #[error("service role is unresolved")]
    Unresolved,
    #[error("service delegation denied or revoked")]
    Denied,
    #[error("nested call cycle or depth exceeded")]
    Cycle,
    #[error("service call deadline expired")]
    Deadline,
    #[error("service owner or provider generation is stale")]
    Stale,
    #[error("service queue or byte budget exceeded")]
    Capacity,
    #[error("service schema or payload is invalid")]
    Invalid,
}

impl DelegationContext {
    /// Inputs come from authenticated host bindings/grants, never plugin-provided authority.
    pub fn child(
        &self,
        provider: PluginInstanceId,
        operation: &NamespacedKind,
        provider_operations: &BTreeSet<NamespacedKind>,
        requested_deadline: u64,
        now: u64,
    ) -> Result<Self, ServiceCallError> {
        if now >= self.deadline_host_ticks || now >= requested_deadline {
            return Err(ServiceCallError::Deadline);
        }
        if self.call_path.len() >= 16 || self.call_path.contains(&provider) {
            return Err(ServiceCallError::Cycle);
        }
        if !self.allowed_operations.contains(operation) || !provider_operations.contains(operation)
        {
            return Err(ServiceCallError::Denied);
        }
        let mut call_path = self.call_path.clone();
        call_path.push(provider);
        Ok(Self {
            owner: self.owner.clone(),
            deadline_host_ticks: self.deadline_host_ticks.min(requested_deadline),
            call_path,
            allowed_operations: self
                .allowed_operations
                .intersection(provider_operations)
                .cloned()
                .collect(),
        })
    }
    pub fn validate_fence(
        &self,
        epoch: ActivationEpoch,
        generation: SessionGeneration,
        grants: GrantRevision,
    ) -> Result<(), ServiceCallError> {
        if self.owner.activation_epoch != epoch || self.owner.caller_generation != generation {
            return Err(ServiceCallError::Stale);
        }
        if self.owner.grant_revision != grants {
            return Err(ServiceCallError::Denied);
        }
        Ok(())
    }
}

/// An authenticated, instance-scoped handle. Implementations resolve role and intersect
/// authority again at dispatch. Completion is polled as a job; this port cannot block scheduler.
pub trait PluginServiceClient: Send + Sync {
    fn start(
        &self,
        role: &NamespacedKind,
        operation: &NamespacedKind,
        schema: &SchemaRef,
        input: serde_json::Value,
        context: &DelegationContext,
    ) -> Result<String, ServiceCallError>;
    fn poll_job(
        &self,
        job: &str,
        owner: &ServiceOwner,
    ) -> Result<ServiceJobState, ServiceCallError>;
    /// Release a terminal result lease; pending work must be cancelled first.
    fn release_job(&self, job: &str, owner: &ServiceOwner) -> Result<(), ServiceCallError>;
    fn cancel_owner(&self, owner: &ServiceOwner) -> Result<(), ServiceCallError>;
    fn subscribe(
        &self,
        role: &NamespacedKind,
        owner: &ServiceOwner,
        maximum_items: usize,
    ) -> Result<String, ServiceCallError>;
    fn unsubscribe(&self, lease: &str, owner: &ServiceOwner) -> Result<(), ServiceCallError>;
    /// Snapshot/delta delivery is bounded. A gap or changed generation requires resubscribe.
    fn poll_subscription(
        &self,
        lease: &str,
        owner: &ServiceOwner,
    ) -> Result<Option<ServiceUpdate>, ServiceCallError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceJobState {
    Pending,
    Completed {
        schema: SchemaRef,
        value: serde_json::Value,
    },
    Failed {
        code: NamespacedKind,
    },
    Cancelled,
    UnknownOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceUpdateKind {
    Snapshot,
    Delta,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceUpdate {
    pub cursor: SubscriptionCursor,
    pub kind: ServiceUpdateKind,
    pub schema: SchemaRef,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionCursor {
    pub provider_generation: SessionGeneration,
    pub revision: u64,
    pub sequence: u64,
}

impl SubscriptionCursor {
    /// A gap requires a new snapshot; no guessed delta replay or implicit read acknowledgement.
    pub fn advance(&mut self, next: Self) -> Result<(), ServiceCallError> {
        if next.provider_generation != self.provider_generation
            || next.revision < self.revision
            || self.sequence.checked_add(1) != Some(next.sequence)
        {
            return Err(ServiceCallError::Stale);
        }
        *self = next;
        Ok(())
    }
}
