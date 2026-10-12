// SPDX-License-Identifier: GPL-3.0-or-later
//! Application-facing apply use case port implemented by Host Lifecycle.

use deckweave_domain::ids::{
    DedupEpoch, DraftId, DraftVersion, IdempotencyKey, OperationId, PrincipalId,
};
use deckweave_workspace::{ActiveSelection, ApplyOperationSnapshot};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanApplyRequest {
    pub draft_id: DraftId,
    pub expected_draft_version: DraftVersion,
    pub expected_active: Option<ActiveSelection>,
    #[serde(default)]
    pub allow_disruption: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPlanPreview {
    pub plan_hash: String,
    pub expires_at_monotonic_millis: u64,
    pub draft_id: DraftId,
    pub draft_version: DraftVersion,
    pub requires_disruption: bool,
    pub disruption_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecuteApplyRequest {
    pub principal_id: PrincipalId,
    pub dedup_epoch: DedupEpoch,
    pub idempotency_key: IdempotencyKey,
    pub draft_id: DraftId,
    pub expected_draft_version: DraftVersion,
    pub expected_active: Option<ActiveSelection>,
    pub allow_disruption: bool,
    pub plan_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplyUseCaseError {
    #[error("apply target was not found")]
    NotFound,
    #[error("apply precondition changed: {0}")]
    PreconditionChanged(String),
    #[error("apply plan is invalid or expired: {0}")]
    InvalidPlan(String),
    #[error("idempotency key belongs to another payload")]
    IdempotencyConflict,
    #[error("idempotency window is unknown or expired")]
    IdempotencyExpired,
    #[error("apply operation capacity is exhausted")]
    CapacityExceeded,
    #[error("apply requires startup recovery: {0}")]
    RecoveryRequired(String),
    #[error("apply service is unavailable: {0}")]
    Unavailable(String),
}

pub trait ApplyUseCase: Send + Sync {
    fn plan_apply(&self, request: &PlanApplyRequest)
    -> Result<ApplyPlanPreview, ApplyUseCaseError>;
    fn execute_apply(
        &self,
        request: &ExecuteApplyRequest,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError>;
    fn operation(
        &self,
        operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError>;
}

#[derive(Debug, Default)]
pub struct UnavailableApplyUseCase;

impl ApplyUseCase for UnavailableApplyUseCase {
    fn plan_apply(
        &self,
        _request: &PlanApplyRequest,
    ) -> Result<ApplyPlanPreview, ApplyUseCaseError> {
        Err(ApplyUseCaseError::Unavailable(
            "apply coordinator is not configured".into(),
        ))
    }

    fn execute_apply(
        &self,
        _request: &ExecuteApplyRequest,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        Err(ApplyUseCaseError::Unavailable(
            "apply coordinator is not configured".into(),
        ))
    }

    fn operation(
        &self,
        _operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        Err(ApplyUseCaseError::Unavailable(
            "apply coordinator is not configured".into(),
        ))
    }
}
