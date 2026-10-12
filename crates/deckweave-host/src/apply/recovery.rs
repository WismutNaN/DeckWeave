// SPDX-License-Identifier: GPL-3.0-or-later
//! Startup reconciliation of active, superseded and never-committed apply work.

use super::*;
use deckweave_workspace::ApplyCommitStatus;

impl ApplyCoordinator {
    pub fn recover(
        &self,
        maximum_operations: usize,
    ) -> Result<Vec<ApplyOperationSnapshot>, ApplyUseCaseError> {
        let _execution = self
            .execution
            .lock()
            .map_err(|_| ApplyUseCaseError::Unavailable("apply lock is poisoned".into()))?;
        let active = self
            .operations
            .active_commit()
            .map_err(map_repository_error)?;
        let mut recovered = Vec::new();
        if let Some(commit) = &active {
            let revision = self
                .operations
                .load_candidate_revision(&commit.candidate)
                .map_err(map_repository_error)?;
            validate_recovery_revision(commit, &revision)?;
            let compiled = match compiled_plan_from_commit(commit)? {
                Some(compiled) => compiled,
                None => self
                    .planning
                    .compile_recovery(
                        &revision,
                        &commit.candidate_sha256,
                        &commit.package_lock_sha256,
                    )
                    .map_err(|error| ApplyUseCaseError::RecoveryRequired(error.to_string()))?,
            };
            validate_recovery_plan(commit, &revision, &compiled)?;
            let outcome = self
                .runtime
                .recover_committed(commit, &compiled)
                .unwrap_or_else(|error| RuntimeCommitOutcome::Degraded(error.to_string()));
            recovered.push(self.finish_committed(&commit.operation_id, outcome)?);
        }
        for pending in self
            .operations
            .recoverable_operations(maximum_operations)
            .map_err(map_repository_error)?
        {
            if active
                .as_ref()
                .is_some_and(|commit| commit.operation_id == pending.intent.operation_id)
            {
                continue;
            }
            if pending.commit_status == ApplyCommitStatus::Committed {
                recovered.push(
                    self.operations
                        .finish_apply(
                            &pending.intent.operation_id,
                            ApplyOperationState::Superseded,
                            ApplyRuntimeHealth::Inactive,
                            Some(
                                "durable commit was superseded before activation outcome was recorded"
                                    .into(),
                            ),
                        )
                        .map_err(map_repository_error)?,
                );
                continue;
            }
            self.runtime
                .abort_before_commit(&pending.intent.operation_id)
                .map_err(|error| ApplyUseCaseError::RecoveryRequired(error.to_string()))?;
            recovered.push(
                self.operations
                    .finish_apply(
                        &pending.intent.operation_id,
                        ApplyOperationState::Failed,
                        ApplyRuntimeHealth::Inactive,
                        Some("host restarted before durable commit".into()),
                    )
                    .map_err(map_repository_error)?,
            );
        }
        Ok(recovered)
    }
}
