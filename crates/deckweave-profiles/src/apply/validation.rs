// SPDX-License-Identifier: GPL-3.0-or-later
//! Apply document and transition invariants enforced at the repository boundary.

use super::*;

pub(super) fn validate_intent(intent: &ApplyOperationIntent) -> Result<(), ApplyRepositoryError> {
    if !is_sha256(&intent.payload_sha256)
        || !is_sha256(&intent.plan_hash)
        || !is_sha256(&intent.candidate_sha256)
        || !is_sha256(&intent.package_lock_sha256)
    {
        return Err(ApplyRepositoryError::InvalidDocument(
            "apply intent contains invalid hashes or identities".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_commit(commit: &ApplyCommitRecord) -> Result<(), ApplyRepositoryError> {
    if !is_sha256(&commit.plan_hash)
        || !is_sha256(&commit.candidate_sha256)
        || !is_sha256(&commit.package_lock_sha256)
    {
        return Err(ApplyRepositoryError::InvalidDocument(
            "apply commit contains invalid hashes".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_finish(
    current: &ApplyOperationSnapshot,
    state: ApplyOperationState,
    health: ApplyRuntimeHealth,
) -> Result<(), ApplyRepositoryError> {
    let valid = match state {
        ApplyOperationState::Succeeded => {
            current.commit_status == ApplyCommitStatus::Committed
                && health == ApplyRuntimeHealth::Applied
        }
        ApplyOperationState::Degraded => {
            current.commit_status == ApplyCommitStatus::Committed
                && health == ApplyRuntimeHealth::Degraded
        }
        ApplyOperationState::Failed => {
            current.commit_status == ApplyCommitStatus::NotCommitted
                && health == ApplyRuntimeHealth::Inactive
        }
        ApplyOperationState::RecoveryRequired => health == ApplyRuntimeHealth::Inactive,
        ApplyOperationState::Queued
        | ApplyOperationState::Prepared
        | ApplyOperationState::Committed => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ApplyRepositoryError::InvalidTransition(format!(
            "cannot finish {:?}/{:?} as {state:?}/{health:?}",
            current.state, current.commit_status
        )))
    }
}
