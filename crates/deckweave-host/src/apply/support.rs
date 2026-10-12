// SPDX-License-Identifier: GPL-3.0-or-later
//! Stable hashing and boundary error translation for the apply coordinator.

use super::*;
use deckweave_automation::{
    COMPILED_PLAN_EXTENSION_ID, CompiledExecutionPlan, DurableExecutionPlan,
};
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

pub(super) fn durable_plan_value(
    plan: &CompiledExecutionPlan,
) -> Result<serde_json::Value, ApplyUseCaseError> {
    let sealed = plan
        .seal()
        .map_err(|error| ApplyUseCaseError::InvalidPlan(error.to_string()))?;
    serde_json::to_value(sealed).map_err(|error| ApplyUseCaseError::Unavailable(error.to_string()))
}

pub(super) fn compiled_plan_from_commit(
    commit: &ApplyCommitRecord,
) -> Result<Option<CompiledExecutionPlan>, ApplyUseCaseError> {
    let Some(value) = commit.extensions.get(COMPILED_PLAN_EXTENSION_ID).cloned() else {
        return Ok(None);
    };
    serde_json::from_value::<DurableExecutionPlan>(value)
        .map_err(|error| ApplyUseCaseError::RecoveryRequired(error.to_string()))?
        .verify()
        .map(Some)
        .map_err(|error| ApplyUseCaseError::RecoveryRequired(error.to_string()))
}

pub(super) fn validate_recovery_revision(
    commit: &ApplyCommitRecord,
    revision: &WorkspaceRevision,
) -> Result<(), ApplyUseCaseError> {
    if revision.workspace_id != commit.candidate.workspace_id
        || revision.revision_id != commit.candidate.revision_id
        || hash_json(&revision.package_lock)? != commit.package_lock_sha256
    {
        return Err(ApplyUseCaseError::RecoveryRequired(
            "committed revision does not match its apply record".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_recovery_plan(
    commit: &ApplyCommitRecord,
    revision: &WorkspaceRevision,
    plan: &CompiledExecutionPlan,
) -> Result<(), ApplyUseCaseError> {
    validate_recovery_revision(commit, revision)?;
    for requirement in plan.runtime_requirements() {
        if !revision.package_lock.iter().any(|locked| {
            locked.package_id == requirement.package_id
                && locked.version == requirement.package_version
                && locked.artifact_sha256 == requirement.artifact_sha256
        }) {
            return Err(ApplyUseCaseError::RecoveryRequired(format!(
                "compiled runtime requirement for {} is absent from the committed package lock",
                requirement.package_id
            )));
        }
    }
    Ok(())
}

pub(super) fn map_planning_error(error: ApplyPlanningError) -> ApplyUseCaseError {
    match error {
        ApplyPlanningError::Repository(error) => map_draft_error(error),
        ApplyPlanningError::PreconditionChanged(field) => {
            ApplyUseCaseError::PreconditionChanged(format!("{field:?}"))
        }
        ApplyPlanningError::Compile(error) => ApplyUseCaseError::InvalidPlan(error.to_string()),
        ApplyPlanningError::DisruptionNotAllowed => {
            ApplyUseCaseError::InvalidPlan("disruptive handoff was not allowed".into())
        }
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
