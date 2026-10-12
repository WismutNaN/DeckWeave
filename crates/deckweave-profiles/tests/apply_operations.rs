// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{
    DedupEpoch, DraftId, DraftVersion, IdempotencyKey, OperationId, PrincipalId, WorkspaceId,
    WorkspaceRevisionId,
};
use deckweave_platform::{AtomicFileStore, PublicationError, StorageError};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{JsonProfileStore, ProfileStoreLimits};
use deckweave_workspace::{
    ActiveSelection, ApplyCommitRecord, ApplyCommitStatus, ApplyOperationIntent,
    ApplyOperationRepository, ApplyOperationState, ApplyRepositoryError, ApplyRuntimeHealth,
    WorkspaceRevision,
};
use std::{
    collections::BTreeMap,
    error::Error,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn store(root: &Path, files: Arc<dyn AtomicFileStore>) -> JsonProfileStore {
    JsonProfileStore::new(root.to_path_buf(), files, ProfileStoreLimits::default())
}

fn selection(revision: &str) -> Result<ActiveSelection, Box<dyn Error>> {
    Ok(ActiveSelection {
        workspace_id: WorkspaceId::try_new("workspace-1")?,
        revision_id: WorkspaceRevisionId::try_new(revision)?,
    })
}

fn revision(revision_id: &str) -> Result<WorkspaceRevision, Box<dyn Error>> {
    Ok(WorkspaceRevision {
        schema_version: 1,
        workspace_id: WorkspaceId::try_new("workspace-1")?,
        revision_id: WorkspaceRevisionId::try_new(revision_id)?,
        package_lock: vec![],
        document: serde_json::json!({}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
}

fn intent(operation: &str, revision: &str) -> Result<ApplyOperationIntent, Box<dyn Error>> {
    Ok(ApplyOperationIntent {
        operation_id: OperationId::try_new(operation)?,
        principal_id: PrincipalId::try_new("local-user")?,
        dedup_epoch: DedupEpoch::try_new("epoch-1")?,
        idempotency_key: IdempotencyKey::try_new(operation)?,
        payload_sha256: "a".repeat(64),
        draft_id: DraftId::try_new("draft-1")?,
        draft_version: DraftVersion::try_new(1)?,
        plan_hash: "b".repeat(64),
        candidate: selection(revision)?,
        previous_active: None,
        candidate_sha256: "c".repeat(64),
        package_lock_sha256: "d".repeat(64),
        extensions: BTreeMap::new(),
    })
}

#[test]
fn selection_changes_only_at_commit_and_recovers_after_restart() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let first = store(directory.path(), Arc::new(NativeAtomicFileStore));
    let abandoned = intent("apply-before-commit", "revision-abandoned")?;
    first.save_candidate_revision(&revision("revision-abandoned")?)?;
    first.begin_apply(&abandoned)?;
    first.mark_prepared(&abandoned.operation_id)?;
    assert!(first.active_commit()?.is_none());

    let reopened = store(directory.path(), Arc::new(NativeAtomicFileStore));
    let pending = reopened.recoverable_operations(10)?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].commit_status, ApplyCommitStatus::NotCommitted);
    reopened.finish_apply(
        &abandoned.operation_id,
        ApplyOperationState::Failed,
        ApplyRuntimeHealth::Inactive,
        Some("host stopped before commit".into()),
    )?;

    let committed = intent("apply-after-commit", "revision-committed")?;
    reopened.save_candidate_revision(&revision("revision-committed")?)?;
    reopened.begin_apply(&committed)?;
    reopened.mark_prepared(&committed.operation_id)?;
    reopened.publish_commit(&ApplyCommitRecord::from_intent(&committed))?;

    let after_commit = store(directory.path(), Arc::new(NativeAtomicFileStore));
    let snapshot = after_commit.operation(&committed.operation_id)?;
    assert_eq!(snapshot.state, ApplyOperationState::Committed);
    assert_eq!(snapshot.commit_status, ApplyCommitStatus::Committed);
    assert_eq!(
        after_commit.active_commit()?.map(|record| record.candidate),
        Some(committed.candidate.clone())
    );
    after_commit.finish_apply(
        &committed.operation_id,
        ApplyOperationState::Succeeded,
        ApplyRuntimeHealth::Applied,
        None,
    )?;
    assert!(after_commit.recoverable_operations(10)?.is_empty());
    Ok(())
}

#[derive(Clone, Copy)]
enum CommitFault {
    LostReplyAfterWrite,
    UnknownWithoutWrite,
    FailStatusAfterCommit,
}

struct FaultingStore {
    fault: CommitFault,
    commit_written: AtomicBool,
}

impl FaultingStore {
    fn new(fault: CommitFault) -> Self {
        Self {
            fault,
            commit_written: AtomicBool::new(false),
        }
    }
}

impl AtomicFileStore for FaultingStore {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), PublicationError> {
        let is_commit = target.ends_with(Path::new("active/commit.json"));
        let is_status = target.ends_with(Path::new("status.json"));
        if is_commit && matches!(self.fault, CommitFault::UnknownWithoutWrite) {
            return Err(PublicationError::unknown(
                target.to_path_buf(),
                target.with_extension("staged"),
                StorageError::Unavailable("injected replace failure".into()),
                "injected reconciliation failure",
            ));
        }
        if is_status
            && self.commit_written.load(Ordering::SeqCst)
            && matches!(self.fault, CommitFault::FailStatusAfterCommit)
        {
            return Err(PublicationError::not_published(StorageError::Unavailable(
                "injected status failure".into(),
            )));
        }
        NativeAtomicFileStore.write_atomic(target, bytes, backup)?;
        if is_commit {
            self.commit_written.store(true, Ordering::SeqCst);
            if matches!(self.fault, CommitFault::LostReplyAfterWrite) {
                return Err(PublicationError::unknown(
                    target.to_path_buf(),
                    target.with_extension("staged"),
                    StorageError::Unavailable("injected lost reply".into()),
                    "injected reconciliation failure",
                ));
            }
        }
        Ok(())
    }
}

fn prepared_with_fault(
    root: &Path,
    fault: CommitFault,
) -> Result<(JsonProfileStore, ApplyOperationIntent), Box<dyn Error>> {
    let repository = store(root, Arc::new(FaultingStore::new(fault)));
    let intent = intent("apply-fault", "revision-fault")?;
    repository.save_candidate_revision(&revision("revision-fault")?)?;
    repository.begin_apply(&intent)?;
    repository.mark_prepared(&intent.operation_id)?;
    Ok((repository, intent))
}

#[test]
fn lost_commit_reply_is_reconciled_from_disk() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (repository, intent) =
        prepared_with_fault(directory.path(), CommitFault::LostReplyAfterWrite)?;
    let snapshot = repository.publish_commit(&ApplyCommitRecord::from_intent(&intent))?;
    assert_eq!(snapshot.commit_status, ApplyCommitStatus::Committed);
    assert_eq!(snapshot.state, ApplyOperationState::Committed);
    Ok(())
}

#[test]
fn unknown_commit_without_new_head_requires_recovery() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (repository, intent) =
        prepared_with_fault(directory.path(), CommitFault::UnknownWithoutWrite)?;
    assert!(matches!(
        repository.publish_commit(&ApplyCommitRecord::from_intent(&intent)),
        Err(ApplyRepositoryError::PublicationUnknown(_))
    ));
    assert!(repository.active_commit()?.is_none());
    let snapshot = repository.operation(&intent.operation_id)?;
    assert_eq!(snapshot.state, ApplyOperationState::Prepared);
    assert_eq!(snapshot.commit_status, ApplyCommitStatus::NotCommitted);
    Ok(())
}

#[test]
fn committed_head_survives_lost_status_write_for_startup_recovery() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (repository, intent) =
        prepared_with_fault(directory.path(), CommitFault::FailStatusAfterCommit)?;
    assert!(matches!(
        repository.publish_commit(&ApplyCommitRecord::from_intent(&intent)),
        Err(ApplyRepositoryError::Unavailable(_))
    ));

    let reopened = store(directory.path(), Arc::new(NativeAtomicFileStore));
    let snapshot = reopened.operation(&intent.operation_id)?;
    assert_eq!(snapshot.state, ApplyOperationState::Prepared);
    assert_eq!(snapshot.commit_status, ApplyCommitStatus::Committed);
    assert_eq!(reopened.recoverable_operations(10)?.len(), 1);
    reopened.publish_commit(&ApplyCommitRecord::from_intent(&intent))?;
    assert_eq!(
        reopened.operation(&intent.operation_id)?.state,
        ApplyOperationState::Committed
    );
    Ok(())
}
