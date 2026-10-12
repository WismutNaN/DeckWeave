// SPDX-License-Identifier: GPL-3.0-or-later
//! Stable hashing and boundary error translation for the apply coordinator.

use super::*;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub(super) fn operation_id(
    request: &ExecuteApplyRequest,
) -> Result<OperationId, ApplyUseCaseError> {
    #[derive(Serialize)]
    struct Key<'a> {
        principal: &'a str,
        epoch: &'a str,
        key: &'a str,
    }
    let digest = hash_json(&Key {
        principal: request.principal_id.as_str(),
        epoch: request.dedup_epoch.as_str(),
        key: request.idempotency_key.as_str(),
    })?;
    OperationId::try_new(format!("apply-{digest}"))
        .map_err(|error| ApplyUseCaseError::Unavailable(error.to_string()))
}

pub(super) fn hash_json(value: &impl Serialize) -> Result<String, ApplyUseCaseError> {
    serde_json::to_vec(value)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| ApplyUseCaseError::Unavailable(error.to_string()))
}

pub(super) fn map_planning_error(error: ApplyPlanningError) -> ApplyUseCaseError {
    match error {
        ApplyPlanningError::Repository(error) => map_draft_error(error),
        ApplyPlanningError::PreconditionChanged(field) => {
            ApplyUseCaseError::PreconditionChanged(format!("{field:?}"))
        }
        ApplyPlanningError::Compile(error) => ApplyUseCaseError::InvalidPlan(error.to_string()),
        other => ApplyUseCaseError::Unavailable(other.to_string()),
    }
}

pub(super) fn map_draft_error(error: DraftRepositoryError) -> ApplyUseCaseError {
    match error {
        DraftRepositoryError::NotFound => ApplyUseCaseError::NotFound,
        DraftRepositoryError::VersionConflict { expected, current } => {
            ApplyUseCaseError::PreconditionChanged(format!("draft {expected}/{current}"))
        }
        DraftRepositoryError::IdempotencyConflict => ApplyUseCaseError::IdempotencyConflict,
        DraftRepositoryError::IdempotencyExpired => ApplyUseCaseError::IdempotencyExpired,
        DraftRepositoryError::ReceiptQuota => ApplyUseCaseError::CapacityExceeded,
        DraftRepositoryError::InvalidCandidate(reason)
        | DraftRepositoryError::InvalidCommand(reason) => ApplyUseCaseError::InvalidPlan(reason),
        DraftRepositoryError::Unavailable(reason) => ApplyUseCaseError::Unavailable(reason),
    }
}

pub(super) fn map_repository_error(error: ApplyRepositoryError) -> ApplyUseCaseError {
    match error {
        ApplyRepositoryError::NotFound => ApplyUseCaseError::NotFound,
        ApplyRepositoryError::IdempotencyConflict => ApplyUseCaseError::IdempotencyConflict,
        ApplyRepositoryError::CapacityExceeded => ApplyUseCaseError::CapacityExceeded,
        ApplyRepositoryError::PublicationUnknown(reason) => {
            ApplyUseCaseError::RecoveryRequired(reason)
        }
        ApplyRepositoryError::InvalidTransition(reason)
        | ApplyRepositoryError::InvalidDocument(reason) => {
            ApplyUseCaseError::PreconditionChanged(reason)
        }
        ApplyRepositoryError::Unavailable(reason) => ApplyUseCaseError::Unavailable(reason),
    }
}
