// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn retired_claims_and_terminal_apply_are_collected_without_losing_pending_or_history()
-> Result<(), Box<dyn Error>> {
    use deckweave_domain::ids::{OperationId, WorkspaceRevisionId};
    use deckweave_workspace::{
        ActiveSelection, ApplyOperationIntent, ApplyOperationRepository, ApplyOperationState,
        ApplyRuntimeHealth, CommandClaim, CommandClaimRepository,
    };
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(TestClock::new(100));
    let store = JsonProfileStore::with_mutation_clock(
        directory.path().into(),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits {
            dedup_window_seconds: 10,
            maximum_archived_apply_operations: 1,
            ..Default::default()
        },
        clock.clone(),
    );
    let mut ids = Vec::new();
    for name in ["terminal", "terminal-other", "pending"] {
        let key = mutation_key(&store, name)?;
        store.claim_command(&CommandClaim {
            key: key.clone(),
            payload_fingerprint: "a".repeat(64),
        })?;
        let operation_id = OperationId::try_new(name)?;
        let intent = ApplyOperationIntent {
            operation_id: operation_id.clone(),
            principal_id: key.principal_id,
            dedup_epoch: key.dedup_epoch,
            idempotency_key: key.idempotency_key,
            payload_sha256: "a".repeat(64),
            draft_id: DraftId::try_new("maintenance")?,
            draft_version: DraftVersion::try_new(1)?,
            plan_hash: "b".repeat(64),
            candidate: ActiveSelection {
                workspace_id: WorkspaceId::try_new("workspace")?,
                revision_id: WorkspaceRevisionId::try_new(name)?,
            },
            previous_active: None,
            allow_disruption: false,
            candidate_sha256: "c".repeat(64),
            package_lock_sha256: "d".repeat(64),
            extensions: BTreeMap::new(),
        };
        store.begin_apply(&intent)?;
        if name.starts_with("terminal") {
            store.finish_apply(
                &operation_id,
                ApplyOperationState::Failed,
                ApplyRuntimeHealth::Inactive,
                None,
            )?;
        }
        ids.push(operation_id);
    }
    assert_eq!(store.archive_terminal_operations()?, 0);
    assert_eq!(store.compact_command_claims()?, 0);
    clock.set(110);
    store.open_mutation_window(&PrincipalId::try_new("local-user")?)?;
    assert_eq!(store.archive_terminal_operations()?, 1);
    assert_eq!(store.archive_terminal_operations()?, 0);
    assert_eq!(store.compact_command_claims()?, 2);
    assert_eq!(store.operation(&ids[0])?.state, ApplyOperationState::Failed);
    assert_eq!(store.operation(&ids[1])?.state, ApplyOperationState::Failed);
    assert_eq!(store.recoverable_operations(4)?.len(), 1);
    let reopened = native_store(directory.path());
    assert_eq!(
        reopened.operation(&ids[0])?.state,
        ApplyOperationState::Failed
    );
    assert_eq!(
        reopened.recoverable_operations(4)?[0].intent.operation_id,
        ids[2]
    );
    Ok(())
}

#[test]
fn checkpoint_write_failures_preserve_selected_draft_and_retry_receipt()
-> Result<(), Box<dyn Error>> {
    for fail_on in [1, 2] {
        let directory = tempfile::tempdir()?;
        let original = draft()?;
        let setup = native_store(directory.path());
        setup.create_draft(&original)?;
        let key = mutation_key(&setup, "checkpoint-fault")?;
        let command = DraftCommand::Set {
            pointer: "/title".into(),
            value: serde_json::json!("kept"),
        };
        let saved =
            setup.mutate_draft(&original.draft_id, key.clone(), original.version, &command)?;
        let failing = JsonProfileStore::new(
            directory.path().into(),
            Arc::new(FailOnCall {
                call: AtomicUsize::new(0),
                fail_on,
            }),
            ProfileStoreLimits::default(),
        );
        assert!(failing.compact_draft(&original.draft_id).is_err());
        let recovered = native_store(directory.path());
        assert_eq!(recovered.load_draft(&original.draft_id)?.draft, saved.draft);
        assert!(
            recovered
                .mutate_draft(&original.draft_id, key, original.version, &command)?
                .replayed
        );
        recovered.compact_draft(&original.draft_id)?;
        assert_eq!(
            native_store(directory.path())
                .load_draft(&original.draft_id)?
                .draft,
            saved.draft
        );
    }
    Ok(())
}

#[test]
fn checkpoint_keeps_receipt_snapshots_and_collects_only_retired_epoch() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(TestClock::new(100));
    let limits = ProfileStoreLimits {
        dedup_window_seconds: 10,
        ..Default::default()
    };
    let store = JsonProfileStore::with_mutation_clock(
        directory.path().into(),
        Arc::new(NativeAtomicFileStore),
        limits.clone(),
        clock.clone(),
    );
    let mut current = draft()?;
    store.create_draft(&current)?;
    let mut saved = Vec::new();
    for index in 0..12 {
        let key = mutation_key(&store, &format!("compact-{index}"))?;
        let command = DraftCommand::Set {
            pointer: "/title".into(),
            value: serde_json::json!(index),
        };
        let version = current.version;
        let result = store.mutate_draft(&current.draft_id, key.clone(), version, &command)?;
        saved.push((key, version, command, result.clone()));
        current = result.draft;
    }
    store.compact_draft(&current.draft_id)?;
    for (key, version, command, result) in &saved {
        let replay = store.mutate_draft(&current.draft_id, key.clone(), *version, command)?;
        assert!(replay.replayed);
        assert_eq!(replay.draft, result.draft);
    }
    clock.set(110);
    // Истечение без ротации ещё не разрешает GC: часы могут перевести назад.
    store.compact_draft(&current.draft_id)?;
    clock.set(105);
    let (key, version, command, _) = &saved[0];
    assert!(
        store
            .mutate_draft(&current.draft_id, key.clone(), *version, command)?
            .replayed
    );
    clock.set(110);
    store.open_mutation_window(&PrincipalId::try_new("local-user")?)?;
    assert!(store.compact_draft(&current.draft_id)? >= 12);
    drop(store);
    let reopened = JsonProfileStore::with_mutation_clock(
        directory.path().into(),
        Arc::new(NativeAtomicFileStore),
        limits,
        clock,
    );
    assert_eq!(reopened.load_draft(&current.draft_id)?.draft, current);
    assert!(matches!(
        reopened.mutate_draft(&current.draft_id, key.clone(), *version, command),
        Err(DraftMutationError::IdempotencyExpired)
    ));
    Ok(())
}
