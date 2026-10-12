// SPDX-License-Identifier: GPL-3.0-or-later
//! Durable apply selection documents and the repository boundary used by Host Lifecycle.

use crate::WorkspaceRevision;
use deckweave_domain::ids::{DraftId, DraftVersion, OperationId, WorkspaceId, WorkspaceRevisionId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSelection {
    pub workspace_id: WorkspaceId,
    pub revision_id: WorkspaceRevisionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyOperationState {
    Queued,
    Prepared,
    Committed,
    Succeeded,
    Failed,
    Degraded,
    RecoveryRequired,
}

impl ApplyOperationState {
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Degraded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyCommitStatus {
    NotCommitted,
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyRuntimeHealth {
    Inactive,
    Applied,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOperationIntent {
    pub operation_id: OperationId,
    pub principal_id: deckweave_domain::ids::PrincipalId,
    pub dedup_epoch: deckweave_domain::ids::DedupEpoch,
    pub idempotency_key: deckweave_domain::ids::IdempotencyKey,
    pub payload_sha256: String,
    pub draft_id: DraftId,
    pub draft_version: DraftVersion,
    pub plan_hash: String,
    pub candidate: ActiveSelection,
    pub previous_active: Option<ActiveSelection>,
    pub candidate_sha256: String,
    pub package_lock_sha256: String,
    #[serde(default)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCommitRecord {
    pub operation_id: OperationId,
    pub candidate: ActiveSelection,
    pub previous_active: Option<ActiveSelection>,
    pub plan_hash: String,
    pub candidate_sha256: String,
    pub package_lock_sha256: String,
    #[serde(default)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl ApplyCommitRecord {
    #[must_use]
    pub fn from_intent(intent: &ApplyOperationIntent) -> Self {
        Self {
            operation_id: intent.operation_id.clone(),
            candidate: intent.candidate.clone(),
            previous_active: intent.previous_active.clone(),
            plan_hash: intent.plan_hash.clone(),
            candidate_sha256: intent.candidate_sha256.clone(),
            package_lock_sha256: intent.package_lock_sha256.clone(),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOperationSnapshot {
    pub intent: ApplyOperationIntent,
    pub state: ApplyOperationState,
    pub commit_status: ApplyCommitStatus,
    pub runtime_health: ApplyRuntimeHealth,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyBegin {
    pub snapshot: ApplyOperationSnapshot,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplyRepositoryError {
    #[error("apply operation was not found")]
    NotFound,
    #[error("idempotency key belongs to another apply payload")]
    IdempotencyConflict,
    #[error("apply operation transition is invalid: {0}")]
    InvalidTransition(String),
    #[error("apply operation capacity is exhausted")]
    CapacityExceeded,
    #[error("apply repository document is invalid: {0}")]
    InvalidDocument(String),
    #[error("apply publication outcome requires recovery: {0}")]
    PublicationUnknown(String),
    #[error("apply repository is unavailable: {0}")]
    Unavailable(String),
}

pub trait ApplyOperationRepository: Send + Sync {
    fn save_candidate_revision(
        &self,
        revision: &WorkspaceRevision,
    ) -> Result<(), ApplyRepositoryError>;
    fn load_candidate_revision(
        &self,
        selection: &ActiveSelection,
    ) -> Result<WorkspaceRevision, ApplyRepositoryError>;
    fn begin_apply(
        &self,
        intent: &ApplyOperationIntent,
    ) -> Result<ApplyBegin, ApplyRepositoryError>;
    fn mark_prepared(
        &self,
        operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError>;
    fn publish_commit(
        &self,
        commit: &ApplyCommitRecord,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError>;
    fn finish_apply(
        &self,
        operation_id: &OperationId,
        state: ApplyOperationState,
        runtime_health: ApplyRuntimeHealth,
        detail: Option<String>,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError>;
    fn operation(
        &self,
        operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError>;
    fn active_commit(&self) -> Result<Option<ApplyCommitRecord>, ApplyRepositoryError>;
    fn recoverable_operations(
        &self,
        maximum: usize,
    ) -> Result<Vec<ApplyOperationSnapshot>, ApplyRepositoryError>;
}
