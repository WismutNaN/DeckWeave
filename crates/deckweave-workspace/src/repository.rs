// SPDX-License-Identifier: GPL-3.0-or-later
use crate::DraftCommand;
use deckweave_domain::ids::{
    DedupEpoch, DraftId, DraftVersion, IdempotencyKey, PrincipalId, WorkspaceId,
    WorkspaceRevisionId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationKey {
    pub principal_id: PrincipalId,
    pub dedup_epoch: DedupEpoch,
    pub idempotency_key: IdempotencyKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationWindow {
    pub principal_id: PrincipalId,
    pub dedup_epoch: DedupEpoch,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftView {
    pub draft_id: DraftId,
    pub workspace_id: WorkspaceId,
    pub base_revision_id: Option<WorkspaceRevisionId>,
    pub version: DraftVersion,
    pub document: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DraftRepositoryError {
    #[error("draft was not found")]
    NotFound,
    #[error("draft version conflict: expected {expected}, current {current}")]
    VersionConflict { expected: u64, current: u64 },
    #[error("idempotency key belongs to another payload")]
    IdempotencyConflict,
    #[error("idempotency window is unknown or expired")]
    IdempotencyExpired,
    #[error("mutation receipt quota is exhausted")]
    ReceiptQuota,
    #[error("draft command is invalid: {0}")]
    InvalidCommand(String),
    #[error("draft store is unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredDraftMutation {
    /// Snapshot published with this receipt, including on an idempotent replay.
    pub draft: DraftView,
    pub key: MutationKey,
    pub before_version: DraftVersion,
    pub after_version: DraftVersion,
    pub document_sha256: String,
    pub replayed: bool,
}

pub trait DraftRepository: Send + Sync {
    fn open_mutation_window(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<MutationWindow, DraftRepositoryError>;
    fn get_draft(&self, draft_id: &DraftId) -> Result<DraftView, DraftRepositoryError>;
    fn preview_draft_mutation(
        &self,
        draft_id: &DraftId,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<DraftView, DraftRepositoryError>;
    fn mutate_draft(
        &self,
        draft_id: &DraftId,
        key: MutationKey,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<StoredDraftMutation, DraftRepositoryError>;
}
