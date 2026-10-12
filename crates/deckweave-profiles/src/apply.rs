// SPDX-License-Identifier: GPL-3.0-or-later
//! JSON adapter for durable apply intents, status and the active commit pointer.

use super::*;
use deckweave_domain::ids::OperationId;
use deckweave_platform::PublicationOutcome;
use deckweave_workspace::{
    ApplyBegin, ApplyCommitRecord, ApplyCommitStatus, ApplyOperationIntent,
    ApplyOperationRepository, ApplyOperationSnapshot, ApplyOperationState, ApplyRepositoryError,
    ApplyRuntimeHealth, WorkspaceRevision,
};
mod document;
mod validation;
use validation::{validate_commit, validate_finish, validate_intent};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredApplyStatus {
    operation_id: OperationId,
    state: ApplyOperationState,
    runtime_health: ApplyRuntimeHealth,
    detail: Option<String>,
}

impl ApplyOperationRepository for JsonProfileStore {
    fn save_candidate_revision(
        &self,
        revision: &WorkspaceRevision,
    ) -> Result<(), ApplyRepositoryError> {
        ProfileStore::save_revision(self, revision)
            .map(|_| ())
            .map_err(map_profile_error)
    }

    fn begin_apply(
        &self,
        intent: &ApplyOperationIntent,
    ) -> Result<ApplyBegin, ApplyRepositoryError> {
        validate_intent(intent)?;
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ApplyRepositoryError::Unavailable("store lock is poisoned".into()))?;
        let intent_path = self.apply_intent_path(&intent.operation_id);
        let replayed = if intent_path.exists() {
            let stored: ApplyOperationIntent = self.read_apply_document(&intent_path)?;
            if stored != *intent {
                return Err(ApplyRepositoryError::IdempotencyConflict);
            }
            true
        } else {
            if operation_directory_count(&self.apply_operations_directory())?
                >= self.limits.maximum_apply_operations
            {
                return Err(ApplyRepositoryError::CapacityExceeded);
            }
            self.write_apply_document(&intent_path, intent, None)?;
            false
        };
        let status_path = self.apply_status_path(&intent.operation_id);
        match self.read_apply_document_with_backup::<StoredApplyStatus>(&status_path) {
            Ok(status) if status.operation_id == intent.operation_id => {}
            Ok(_) => {
                return Err(ApplyRepositoryError::InvalidDocument(
                    "operation status identity mismatch".into(),
                ));
            }
            Err(ApplyRepositoryError::NotFound) => {
                self.write_apply_document(
                    &status_path,
                    &StoredApplyStatus {
                        operation_id: intent.operation_id.clone(),
                        state: ApplyOperationState::Queued,
                        runtime_health: ApplyRuntimeHealth::Inactive,
                        detail: None,
                    },
                    Some(&status_path.with_extension("json.bak")),
                )?;
            }
            Err(error) => return Err(error),
        }
        Ok(ApplyBegin {
            snapshot: self.operation_inner(&intent.operation_id)?,
            replayed,
        })
    }

    fn mark_prepared(
        &self,
        operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        let _guard = self.lock_apply_store()?;
        let current = self.operation_inner(operation_id)?;
        match current.state {
            ApplyOperationState::Queued => self.write_status(
                operation_id,
                ApplyOperationState::Prepared,
                ApplyRuntimeHealth::Inactive,
                None,
            )?,
            ApplyOperationState::Prepared
            | ApplyOperationState::Committed
            | ApplyOperationState::Succeeded
            | ApplyOperationState::Degraded => return Ok(current),
            ApplyOperationState::Failed | ApplyOperationState::RecoveryRequired => {
                return Err(ApplyRepositoryError::InvalidTransition(format!(
                    "cannot prepare operation in {:?}",
                    current.state
                )));
            }
        }
        self.operation_inner(operation_id)
    }

    fn publish_commit(
        &self,
        commit: &ApplyCommitRecord,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        validate_commit(commit)?;
        let _guard = self.lock_apply_store()?;
        let current = self.operation_inner(&commit.operation_id)?;
        if ApplyCommitRecord::from_intent(&current.intent) != *commit {
            return Err(ApplyRepositoryError::InvalidTransition(
                "commit does not match durable intent".into(),
            ));
        }
        if current.commit_status == ApplyCommitStatus::Committed {
            let active = self.active_commit_inner()?.ok_or_else(|| {
                ApplyRepositoryError::InvalidDocument("committed operation has no head".into())
            })?;
            return if active == *commit {
                if current.state == ApplyOperationState::Prepared {
                    self.write_status(
                        &commit.operation_id,
                        ApplyOperationState::Committed,
                        ApplyRuntimeHealth::Inactive,
                        None,
                    )?;
                }
                self.operation_inner(&commit.operation_id)
            } else {
                Err(ApplyRepositoryError::InvalidDocument(
                    "operation and active commit disagree".into(),
                ))
            };
        }
        if current.state != ApplyOperationState::Prepared {
            return Err(ApplyRepositoryError::InvalidTransition(format!(
                "commit requires prepared state, found {:?}",
                current.state
            )));
        }
        let active = self.active_commit_inner()?;
        if active.as_ref().map(|record| &record.candidate) != commit.previous_active.as_ref() {
            return Err(ApplyRepositoryError::InvalidTransition(
                "active selection changed before commit".into(),
            ));
        }
        let path = self.active_commit_path();
        self.write_apply_document(&path, commit, Some(&path.with_extension("json.bak")))?;
        self.write_status(
            &commit.operation_id,
            ApplyOperationState::Committed,
            ApplyRuntimeHealth::Inactive,
            None,
        )?;
        self.operation_inner(&commit.operation_id)
    }

    fn finish_apply(
        &self,
        operation_id: &OperationId,
        state: ApplyOperationState,
        runtime_health: ApplyRuntimeHealth,
        detail: Option<String>,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        let _guard = self.lock_apply_store()?;
        let current = self.operation_inner(operation_id)?;
        if current.state.is_terminal() {
            if current.state == state
                && current.runtime_health == runtime_health
                && current.detail == detail
            {
                return Ok(current);
            }
            return Err(ApplyRepositoryError::InvalidTransition(
                "terminal operation cannot be rewritten".into(),
            ));
        }
        validate_finish(&current, state, runtime_health)?;
        self.write_status(operation_id, state, runtime_health, detail)?;
        self.operation_inner(operation_id)
    }

    fn operation(
        &self,
        operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        let _guard = self.lock_apply_store()?;
        self.operation_inner(operation_id)
    }

    fn active_commit(&self) -> Result<Option<ApplyCommitRecord>, ApplyRepositoryError> {
        let _guard = self.lock_apply_store()?;
        self.active_commit_inner()
    }

    fn recoverable_operations(
        &self,
        maximum: usize,
    ) -> Result<Vec<ApplyOperationSnapshot>, ApplyRepositoryError> {
        let _guard = self.lock_apply_store()?;
        let directory = self.apply_operations_directory();
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(ApplyRepositoryError::Unavailable(error.to_string())),
        };
        let mut operation_ids = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|error| ApplyRepositoryError::Unavailable(error.to_string()))?;
            if !entry
                .file_type()
                .map_err(|error| ApplyRepositoryError::Unavailable(error.to_string()))?
                .is_dir()
            {
                return Err(ApplyRepositoryError::InvalidDocument(
                    "unexpected file in apply operations directory".into(),
                ));
            }
            let intent_path = entry.path().join("intent.json");
            let intent: ApplyOperationIntent = self.read_apply_document(&intent_path)?;
            operation_ids.push(intent.operation_id);
            if operation_ids.len() > maximum {
                return Err(ApplyRepositoryError::CapacityExceeded);
            }
        }
        operation_ids.sort();
        operation_ids
            .into_iter()
            .map(|id| self.operation_inner(&id))
            .filter(|result| {
                result
                    .as_ref()
                    .map_or(true, |snapshot| !snapshot.state.is_terminal())
            })
            .collect()
    }
}

impl JsonProfileStore {
    fn lock_apply_store(&self) -> Result<std::sync::MutexGuard<'_, ()>, ApplyRepositoryError> {
        self.mutation_lock
            .lock()
            .map_err(|_| ApplyRepositoryError::Unavailable("store lock is poisoned".into()))
    }

    fn operation_inner(
        &self,
        operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        let intent: ApplyOperationIntent =
            self.read_apply_document(&self.apply_intent_path(operation_id))?;
        if intent.operation_id != *operation_id {
            return Err(ApplyRepositoryError::InvalidDocument(
                "operation lookup identity mismatch".into(),
            ));
        }
        let status: StoredApplyStatus = self
            .read_apply_document_with_backup(&self.apply_status_path(operation_id))
            .or_else(|error| match error {
                ApplyRepositoryError::NotFound => Ok(StoredApplyStatus {
                    operation_id: operation_id.clone(),
                    state: ApplyOperationState::Queued,
                    runtime_health: ApplyRuntimeHealth::Inactive,
                    detail: None,
                }),
                other => Err(other),
            })?;
        if status.operation_id != *operation_id {
            return Err(ApplyRepositoryError::InvalidDocument(
                "operation status identity mismatch".into(),
            ));
        }
        let committed = matches!(
            status.state,
            ApplyOperationState::Committed
                | ApplyOperationState::Succeeded
                | ApplyOperationState::Degraded
        ) || self
            .active_commit_inner()?
            .is_some_and(|record| record.operation_id == *operation_id);
        Ok(ApplyOperationSnapshot {
            intent,
            state: status.state,
            commit_status: if committed {
                ApplyCommitStatus::Committed
            } else {
                ApplyCommitStatus::NotCommitted
            },
            runtime_health: status.runtime_health,
            detail: status.detail,
        })
    }

    fn write_status(
        &self,
        operation_id: &OperationId,
        state: ApplyOperationState,
        runtime_health: ApplyRuntimeHealth,
        detail: Option<String>,
    ) -> Result<(), ApplyRepositoryError> {
        let path = self.apply_status_path(operation_id);
        self.write_apply_document(
            &path,
            &StoredApplyStatus {
                operation_id: operation_id.clone(),
                state,
                runtime_health,
                detail,
            },
            Some(&path.with_extension("json.bak")),
        )
    }

    fn active_commit_inner(&self) -> Result<Option<ApplyCommitRecord>, ApplyRepositoryError> {
        let path = self.active_commit_path();
        match self.read_apply_document_with_backup(&path) {
            Ok(record) => Ok(Some(record)),
            Err(ApplyRepositoryError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn operation_directory_count(path: &Path) -> Result<usize, ApplyRepositoryError> {
    match fs::read_dir(path) {
        Ok(entries) => entries
            .map(|entry| {
                entry.map_err(|error| ApplyRepositoryError::Unavailable(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|entries| entries.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(ApplyRepositoryError::Unavailable(error.to_string())),
    }
}

fn map_profile_error(error: ProfileStoreError) -> ApplyRepositoryError {
    match error {
        ProfileStoreError::NotFound => ApplyRepositoryError::NotFound,
        ProfileStoreError::Storage(error) if error.outcome() == PublicationOutcome::Unknown => {
            ApplyRepositoryError::PublicationUnknown(error.to_string())
        }
        other => ApplyRepositoryError::Unavailable(other.to_string()),
    }
}
