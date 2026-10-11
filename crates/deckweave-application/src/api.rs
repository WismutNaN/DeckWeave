// SPDX-License-Identifier: GPL-3.0-or-later
pub use deckweave_domain::ids::PrincipalId;
use deckweave_domain::ids::{
    DedupEpoch, DraftId, DraftVersion, IdempotencyKey, OperationId, TraceId, WorkspaceId,
    WorkspaceRevisionId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::HostSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiAuthScope {
    Read,
    Draft,
    Simulation,
    Apply,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    pub principal_id: PrincipalId,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationKey {
    pub principal_id: PrincipalId,
    pub dedup_epoch: DedupEpoch,
    pub idempotency_key: IdempotencyKey,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DraftCommand {
    Set {
        pointer: String,
        value: serde_json::Value,
    },
    Remove {
        pointer: String,
    },
    ReplaceDocument {
        document: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppQuery {
    Host,
    Draft { draft_id: DraftId },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppQueryResult {
    Host(HostSnapshot),
    Draft(DraftView),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppCommand {
    PatchDraft {
        draft_id: DraftId,
        expected_version: DraftVersion,
        edit: DraftCommand,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope {
    pub request_id: String,
    pub idempotency_key: IdempotencyKey,
    pub dedup_epoch: DedupEpoch,
    pub dry_run: bool,
    pub command: AppCommand,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftMutationReceipt {
    pub key: MutationKey,
    pub before_version: DraftVersion,
    pub after_version: DraftVersion,
    pub document_sha256: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppCommandResult {
    DraftMutation {
        draft: Box<DraftView>,
        receipt: Option<DraftMutationReceipt>,
        dry_run: bool,
    },
    OperationAccepted(OperationReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationReceipt {
    pub operation_id: OperationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    PermissionDenied,
    Unavailable,
    NotFound,
    RevisionConflict,
    IdempotencyConflict,
    InvalidCommand,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Error)]
#[error("{message_key}")]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message_key: String,
    pub details: BTreeMap<String, serde_json::Value>,
    pub retryable: bool,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApiAuthorizationError {
    #[error("principal does not have the required API scope")]
    Denied,
    #[error("API authorization service is unavailable: {0}")]
    Unavailable(String),
}

pub trait ApiAuthorizer: Send + Sync {
    fn authorize(
        &self,
        principal_id: &PrincipalId,
        scope: ApiAuthScope,
    ) -> Result<(), ApiAuthorizationError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DraftStoreError {
    #[error("draft was not found")]
    NotFound,
    #[error("draft version conflict: expected {expected}, current {current}")]
    VersionConflict { expected: u64, current: u64 },
    #[error("idempotency key belongs to another payload")]
    IdempotencyConflict,
    #[error("draft command is invalid: {0}")]
    InvalidCommand(String),
    #[error("draft store is unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDraftMutation {
    pub key: MutationKey,
    pub before_version: DraftVersion,
    pub after_version: DraftVersion,
    pub document_sha256: String,
    pub replayed: bool,
}

pub trait DraftStore: Send + Sync {
    fn get_draft(&self, draft_id: &DraftId) -> Result<DraftView, DraftStoreError>;
    fn preview_draft_mutation(
        &self,
        draft_id: &DraftId,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<DraftView, DraftStoreError>;
    fn mutate_draft(
        &self,
        draft_id: &DraftId,
        key: MutationKey,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<StoredDraftMutation, DraftStoreError>;
}
