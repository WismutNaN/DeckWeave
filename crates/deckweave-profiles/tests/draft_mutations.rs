// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{DraftId, DraftVersion, IdempotencyKey, PrincipalId, WorkspaceId};
use deckweave_platform::{AtomicFileStore, PublicationError, StorageError};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{
    AssetReference, DraftCommand, DraftEditor, DraftMutationError, JsonProfileStore, MutationClock,
    MutationKey, ProfileStore, ProfileStoreError, ProfileStoreLimits, WorkspaceDraft,
};
use deckweave_workspace::{DraftRepository, DraftRepositoryError};

#[path = "support/compaction.rs"]
mod compaction;
#[path = "support/growth.rs"]
mod growth;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

fn draft() -> Result<WorkspaceDraft, Box<dyn Error>> {
    Ok(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("draft-main")?,
        workspace_id: WorkspaceId::try_new("workspace-main")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({"bindings": [], "title": "DeckWeave"}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
}

fn another_draft() -> Result<WorkspaceDraft, Box<dyn Error>> {
    let mut draft = draft()?;
    draft.draft_id = DraftId::try_new("draft-other")?;
    draft.workspace_id = WorkspaceId::try_new("workspace-other")?;
    Ok(draft)
}

fn mutation_key(store: &JsonProfileStore, id: &str) -> Result<MutationKey, Box<dyn Error>> {
    let principal_id = PrincipalId::try_new("local-user")?;
    let window = store.open_mutation_window(&principal_id)?;
    Ok(MutationKey {
        principal_id,
        dedup_epoch: window.dedup_epoch,
        idempotency_key: IdempotencyKey::try_new(id)?,
    })
}

fn native_store(root: &Path) -> JsonProfileStore {
    JsonProfileStore::new(
        root.to_path_buf(),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    )
}

#[test]
fn editor_reopens_current_draft_and_replays_configuration_with_its_lock()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let initial = draft()?;
    assert_eq!(store.ensure_draft(&initial)?.version, initial.version);
    let lock = deckweave_workspace::PackageLockEntry {
        package_id: deckweave_domain::ids::PackageId::try_new("org.example.editor")?,
        version: "1.0.0".into(),
        artifact_sha256: "a".repeat(64),
        source: None,
        extensions: Default::default(),
    };
    let command = DraftCommand::ReplaceConfiguration {
        document: serde_json::json!({"bound":true}),
        package_lock: vec![lock.clone()],
    };
    let key = mutation_key(&store, "editor-config")?;
    let result = store.mutate_draft(&initial.draft_id, key.clone(), initial.version, &command)?;
    let reopened = store.ensure_draft(&initial)?;
    assert_eq!(reopened.document, result.draft.document);
    assert_eq!(reopened.package_lock, vec![lock]);
    let store = native_store(directory.path());
    let replay =
        DraftRepository::mutate_draft(&store, &initial.draft_id, key, initial.version, &command)?;
    assert!(replay.replayed);
    assert_eq!(replay.draft, reopened);
    Ok(())
}

#[test]
fn mutation_is_durable_and_deduplication_precedes_version_check() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let original = draft()?;
    store.create_draft(&original)?;
    let command = DraftCommand::Set {
        pointer: "/title".into(),
        value: serde_json::json!("Durable"),
    };
    let key = mutation_key(&store, "request-1")?;

    let first = store.mutate_draft(&original.draft_id, key.clone(), original.version, &command)?;
    assert!(!first.replayed);
    assert_eq!(first.receipt.after_version.get(), 2);

    let replay = store.mutate_draft(&original.draft_id, key.clone(), original.version, &command)?;
    assert!(replay.replayed);
    assert_eq!(replay.receipt, first.receipt);

    assert!(matches!(
        store.mutate_draft(
            &original.draft_id,
            key,
            original.version,
            &DraftCommand::Set {
                pointer: "/title".into(),
                value: serde_json::json!("Different"),
            },
        ),
        Err(DraftMutationError::IdempotencyConflict)
    ));
    assert!(matches!(
        store.mutate_draft(
            &original.draft_id,
            mutation_key(&store, "request-2")?,
            original.version,
            &command,
        ),
        Err(DraftMutationError::VersionConflict {
            expected: 1,
            current: 2
        })
    ));

    let reopened = native_store(directory.path());
    let loaded = reopened.load_draft(&original.draft_id)?;
    assert_eq!(loaded.draft.version.get(), 2);
    assert_eq!(loaded.draft.document["title"], "Durable");
    Ok(())
}

#[test]
fn undo_and_redo_are_bounded_session_state() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = Arc::new(native_store(directory.path()));
    let original = draft()?;
    store.create_draft(&original)?;
    let mut editor = DraftEditor::open(Arc::clone(&store), original.draft_id.clone(), 2)?;

    editor.apply(
        mutation_key(&store, "apply-1")?,
        original.version,
        DraftCommand::Set {
            pointer: "/count".into(),
            value: serde_json::json!(1),
        },
    )?;
    assert!(editor.has_undo());
    assert!(!editor.has_redo());

    let undo = editor
        .undo(mutation_key(&store, "undo-1")?)?
        .ok_or_else(|| std::io::Error::other("missing undo entry"))?;
    assert_eq!(undo.receipt.after_version.get(), 3);
    assert_eq!(
        store.load_draft(&original.draft_id)?.draft.document,
        original.document
    );
    assert!(editor.has_redo());

    let redo = editor
        .redo(mutation_key(&store, "redo-1")?)?
        .ok_or_else(|| std::io::Error::other("missing redo entry"))?;
    assert_eq!(redo.receipt.after_version.get(), 4);
    assert_eq!(
        store.load_draft(&original.draft_id)?.draft.document["count"],
        1
    );

    let restarted_editor = DraftEditor::open(Arc::clone(&store), original.draft_id, 2)?;
    assert!(!restarted_editor.has_undo());
    assert!(!restarted_editor.has_redo());
    Ok(())
}

struct FailOnCall {
    call: AtomicUsize,
    fail_on: usize,
}

struct TestClock {
    now: AtomicU64,
}

impl TestClock {
    fn new(now: u64) -> Self {
        Self {
            now: AtomicU64::new(now),
        }
    }

    fn set(&self, now: u64) {
        self.now.store(now, Ordering::SeqCst);
    }
}

impl MutationClock for TestClock {
    fn unix_seconds(&self) -> Result<u64, ProfileStoreError> {
        Ok(self.now.load(Ordering::SeqCst))
    }
}

#[test]
fn undo_and_redo_reject_changes_from_another_editor() -> Result<(), Box<dyn Error>> {
    for redo in [false, true] {
        let directory = tempfile::tempdir()?;
        let store = Arc::new(native_store(directory.path()));
        let original = draft()?;
        store.create_draft(&original)?;
        let mut editor = DraftEditor::open(Arc::clone(&store), original.draft_id.clone(), 8)?;
        editor.apply(
            mutation_key(&store, "own-edit")?,
            original.version,
            DraftCommand::Set {
                pointer: "/title".into(),
                value: "Own edit".into(),
            },
        )?;
        if redo {
            editor.undo(mutation_key(&store, "own-undo")?)?;
        }
        let before_external = store.load_draft(&original.draft_id)?.draft;
        store.mutate_draft(
            &original.draft_id,
            mutation_key(&store, "external")?,
            before_external.version,
            &DraftCommand::Set {
                pointer: "/external".into(),
                value: true.into(),
            },
        )?;
        let expected = store.load_draft(&original.draft_id)?.draft;
        let result = if redo {
            editor.redo(mutation_key(&store, "redo")?)
        } else {
            editor.undo(mutation_key(&store, "undo")?)
        };
        assert!(
            matches!(result, Err(DraftMutationError::VersionConflict { .. })),
            "session history must not overwrite another writer, redo={redo}"
        );
        assert_eq!(store.load_draft(&original.draft_id)?.draft, expected);
    }
    Ok(())
}

#[test]
fn accepting_a_new_version_starts_a_new_undo_history() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = Arc::new(native_store(directory.path()));
    let original = draft()?;
    store.create_draft(&original)?;
    let mut editor = DraftEditor::open(Arc::clone(&store), original.draft_id.clone(), 8)?;
    editor.apply(
        mutation_key(&store, "own-edit")?,
        original.version,
        DraftCommand::Set {
            pointer: "/title".into(),
            value: "Own edit".into(),
        },
    )?;
    store.mutate_draft(
        &original.draft_id,
        mutation_key(&store, "external")?,
        DraftVersion::try_new(2)?,
        &DraftCommand::Set {
            pointer: "/external".into(),
            value: true.into(),
        },
    )?;
    let external = store.load_draft(&original.draft_id)?.draft;
    editor.apply(
        mutation_key(&store, "after-refresh")?,
        external.version,
        DraftCommand::Set {
            pointer: "/title".into(),
            value: "After refresh".into(),
        },
    )?;
    editor.undo(mutation_key(&store, "undo")?)?;
    assert_eq!(
        store.load_draft(&original.draft_id)?.draft.document,
        external.document
    );
    assert!(
        !editor.has_undo(),
        "history before another writer must not be reachable"
    );
    Ok(())
}

impl AtomicFileStore for FailOnCall {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), PublicationError> {
        let call = self.call.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(PublicationError::not_published(StorageError::Unavailable(
                "injected head publication failure".into(),
            )));
        }
        NativeAtomicFileStore.write_atomic(target, bytes, backup)
    }
}

#[test]
fn unpublished_transaction_is_ignored_and_retryable() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let original = draft()?;
    let setup = native_store(directory.path());
    setup.create_draft(&original)?;
    let key = mutation_key(&setup, "retry-1")?;
    let failing = JsonProfileStore::new(
        directory.path().to_path_buf(),
        Arc::new(FailOnCall {
            call: AtomicUsize::new(0),
            fail_on: 2,
        }),
        ProfileStoreLimits::default(),
    );
    let command = DraftCommand::Set {
        pointer: "/title".into(),
        value: serde_json::json!("Published after retry"),
    };
    assert!(matches!(
        failing.mutate_draft(&original.draft_id, key.clone(), original.version, &command),
        Err(DraftMutationError::Store(_))
    ));
    let recovered = native_store(directory.path());
    assert_eq!(recovered.load_draft(&original.draft_id)?.draft, original);

    let retry = recovered.mutate_draft(&original.draft_id, key, original.version, &command)?;
    assert!(!retry.replayed);
    assert_eq!(retry.receipt.after_version.get(), 2);
    Ok(())
}

#[test]
fn pending_mutation_recovers_failures_around_head_and_receipt_publication()
-> Result<(), Box<dyn Error>> {
    for fail_on in [3, 4] {
        let directory = tempfile::tempdir()?;
        let original = draft()?;
        let setup = native_store(directory.path());
        setup.create_draft(&original)?;
        let key = mutation_key(&setup, &format!("recover-{fail_on}"))?;
        let failing = JsonProfileStore::new(
            directory.path().to_path_buf(),
            Arc::new(FailOnCall {
                call: AtomicUsize::new(0),
                fail_on,
            }),
            ProfileStoreLimits::default(),
        );
        let command = DraftCommand::Set {
            pointer: "/title".into(),
            value: serde_json::json!(format!("Recovered {fail_on}")),
        };

        assert!(matches!(
            failing.mutate_draft(&original.draft_id, key.clone(), original.version, &command),
            Err(DraftMutationError::Store(_))
        ));

        let reopened = native_store(directory.path());
        let loaded = reopened.load_draft(&original.draft_id)?;
        assert_eq!(loaded.draft.version.get(), 2, "fail_on={fail_on}");
        assert_eq!(
            loaded.draft.document["title"],
            format!("Recovered {fail_on}"),
            "fail_on={fail_on}"
        );
        let replay = reopened.mutate_draft(&original.draft_id, key, original.version, &command)?;
        assert!(replay.replayed, "fail_on={fail_on}");
    }
    Ok(())
}

#[test]
fn mutation_key_is_global_across_drafts_and_survives_restart() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let first_draft = draft()?;
    let second_draft = another_draft()?;
    store.create_draft(&first_draft)?;
    store.create_draft(&second_draft)?;
    let key = mutation_key(&store, "global-key")?;
    let command = DraftCommand::Set {
        pointer: "/title".into(),
        value: serde_json::json!("First target"),
    };
    store.mutate_draft(
        &first_draft.draft_id,
        key.clone(),
        first_draft.version,
        &command,
    )?;

    let reopened = native_store(directory.path());
    assert!(matches!(
        reopened.mutate_draft(&second_draft.draft_id, key, second_draft.version, &command,),
        Err(DraftMutationError::IdempotencyConflict)
    ));
    assert_eq!(
        reopened.load_draft(&second_draft.draft_id)?.draft,
        second_draft
    );
    Ok(())
}

#[test]
fn expired_or_unknown_mutation_windows_are_rejected_before_replay() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(TestClock::new(100));
    let limits = ProfileStoreLimits {
        dedup_window_seconds: 10,
        ..ProfileStoreLimits::default()
    };
    let store = JsonProfileStore::with_mutation_clock(
        directory.path().to_path_buf(),
        Arc::new(NativeAtomicFileStore),
        limits,
        Arc::clone(&clock) as Arc<dyn MutationClock>,
    );
    let original = draft()?;
    store.create_draft(&original)?;
    let key = mutation_key(&store, "expires")?;
    let command = DraftCommand::Set {
        pointer: "/title".into(),
        value: serde_json::json!("Committed"),
    };
    store.mutate_draft(&original.draft_id, key.clone(), original.version, &command)?;

    clock.set(110);
    assert!(matches!(
        store.mutate_draft(&original.draft_id, key, original.version, &command),
        Err(DraftMutationError::IdempotencyExpired)
    ));
    let renewed = store.open_mutation_window(&PrincipalId::try_new("local-user")?)?;
    assert_eq!(renewed.issued_at_unix_seconds, 110);
    Ok(())
}

#[test]
fn receipt_quota_stops_new_mutations_without_changing_the_draft() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let limits = ProfileStoreLimits {
        maximum_mutation_receipts: 1,
        ..ProfileStoreLimits::default()
    };
    let store = JsonProfileStore::new(
        directory.path().to_path_buf(),
        Arc::new(NativeAtomicFileStore),
        limits,
    );
    let original = draft()?;
    store.create_draft(&original)?;
    store.mutate_draft(
        &original.draft_id,
        mutation_key(&store, "quota-first")?,
        original.version,
        &DraftCommand::Set {
            pointer: "/first".into(),
            value: true.into(),
        },
    )?;
    let before_rejected = store.load_draft(&original.draft_id)?.draft;

    assert!(matches!(
        store.mutate_draft(
            &original.draft_id,
            mutation_key(&store, "quota-second")?,
            before_rejected.version,
            &DraftCommand::Set {
                pointer: "/second".into(),
                value: true.into(),
            },
        ),
        Err(DraftMutationError::ReceiptQuota)
    ));
    assert_eq!(store.load_draft(&original.draft_id)?.draft, before_rejected);
    Ok(())
}

#[test]
fn candidate_requires_every_referenced_asset_with_matching_metadata() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let descriptor = store.put_asset(b"candidate asset", "text/plain")?;
    let mut valid = draft()?;
    valid.draft_id = DraftId::try_new("candidate-assets")?;
    valid.assets.push(AssetReference {
        id: descriptor.id.clone(),
        sha256: descriptor.sha256.clone(),
        media_type: descriptor.media_type.clone(),
        size_bytes: descriptor.size_bytes,
        extensions: BTreeMap::new(),
    });
    store.create_draft(&valid)?;
    let candidate = DraftRepository::get_candidate(&store, &valid.draft_id)?;
    assert_eq!(candidate.assets_sha256().len(), 64);

    let mut missing = draft()?;
    missing.draft_id = DraftId::try_new("candidate-missing")?;
    let sha256 = "c".repeat(64);
    missing.assets.push(AssetReference {
        id: deckweave_domain::ids::AssetId::try_new(&sha256)?,
        sha256,
        media_type: "text/plain".into(),
        size_bytes: 1,
        extensions: BTreeMap::new(),
    });
    store.create_draft(&missing)?;
    assert!(matches!(
        DraftRepository::get_candidate(&store, &missing.draft_id),
        Err(DraftRepositoryError::InvalidCandidate(_))
    ));
    Ok(())
}

#[test]
fn corrupt_primary_head_falls_back_to_last_published_backup() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let original = draft()?;
    store.create_draft(&original)?;
    store.mutate_draft(
        &original.draft_id,
        mutation_key(&store, "head-1")?,
        original.version,
        &DraftCommand::Set {
            pointer: "/title".into(),
            value: serde_json::json!("Version two"),
        },
    )?;

    let draft_key = format!("{:x}", Sha256::digest(original.draft_id.as_str()));
    let head_path = directory
        .path()
        .join("drafts")
        .join(draft_key)
        .join("head.json");
    fs::write(head_path, b"{corrupt")?;

    let loaded = store.load_draft(&original.draft_id)?;
    assert!(loaded.recovered_from_backup);
    assert_eq!(loaded.draft, original);

    store.mutate_draft(
        &original.draft_id,
        mutation_key(&store, "head-recovery-write")?,
        original.version,
        &DraftCommand::Set {
            pointer: "/title".into(),
            value: serde_json::json!("Recovered write"),
        },
    )?;
    let reopened = native_store(directory.path()).load_draft(&original.draft_id)?;
    assert!(!reopened.recovered_from_backup);
    assert_eq!(reopened.draft.version.get(), 2);
    assert_eq!(reopened.draft.document["title"], "Recovered write");
    Ok(())
}
