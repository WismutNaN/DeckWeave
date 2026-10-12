// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{
    DedupEpoch, DraftId, DraftVersion, IdempotencyKey, PrincipalId, WorkspaceId,
};
use deckweave_platform::{AtomicFileStore, StorageError};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{
    DraftCommand, DraftEditor, DraftMutationError, JsonProfileStore, MutationKey,
    ProfileStoreLimits, WorkspaceDraft,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
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

fn mutation_key(id: &str) -> Result<MutationKey, Box<dyn Error>> {
    Ok(MutationKey {
        principal_id: PrincipalId::try_new("local-user")?,
        dedup_epoch: DedupEpoch::try_new("boot-1")?,
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
fn mutation_is_durable_and_deduplication_precedes_version_check() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let original = draft()?;
    store.create_draft(&original)?;
    let command = DraftCommand::Set {
        pointer: "/title".into(),
        value: serde_json::json!("Durable"),
    };
    let key = mutation_key("request-1")?;

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
            mutation_key("request-2")?,
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
        mutation_key("apply-1")?,
        original.version,
        DraftCommand::Set {
            pointer: "/count".into(),
            value: serde_json::json!(1),
        },
    )?;
    assert!(editor.has_undo());
    assert!(!editor.has_redo());

    let undo = editor
        .undo(mutation_key("undo-1")?)?
        .ok_or_else(|| std::io::Error::other("missing undo entry"))?;
    assert_eq!(undo.receipt.after_version.get(), 3);
    assert_eq!(
        store.load_draft(&original.draft_id)?.draft.document,
        original.document
    );
    assert!(editor.has_redo());

    let redo = editor
        .redo(mutation_key("redo-1")?)?
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

impl AtomicFileStore for FailOnCall {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), StorageError> {
        let call = self.call.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(StorageError::Unavailable(
                "injected head publication failure".into(),
            ));
        }
        NativeAtomicFileStore.write_atomic(target, bytes, backup)
    }
}

#[test]
fn unpublished_transaction_is_ignored_and_retryable() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let original = draft()?;
    native_store(directory.path()).create_draft(&original)?;
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
    let key = mutation_key("retry-1")?;

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
fn corrupt_primary_head_falls_back_to_last_published_backup() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let original = draft()?;
    store.create_draft(&original)?;
    store.mutate_draft(
        &original.draft_id,
        mutation_key("head-1")?,
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
        mutation_key("head-recovery-write")?,
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
