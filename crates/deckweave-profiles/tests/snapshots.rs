// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{DraftId, PackageId, SnapshotId, WorkspaceId, WorkspaceRevisionId};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{
    JsonProfileStore, PackageLockEntry, ProfileStore, ProfileStoreError, ProfileStoreLimits,
    SnapshotRef, WorkspaceRevision,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, fs, path::Path, sync::Arc};

fn native_store(root: &Path) -> JsonProfileStore {
    JsonProfileStore::new(
        root.to_path_buf(),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    )
}

fn revision() -> Result<WorkspaceRevision, Box<dyn Error>> {
    Ok(WorkspaceRevision {
        schema_version: 1,
        workspace_id: WorkspaceId::try_new("workspace-snapshots")?,
        revision_id: WorkspaceRevisionId::try_new("revision-stable")?,
        package_lock: vec![PackageLockEntry {
            package_id: PackageId::try_new("org.deckweave.fixture.virtual")?,
            version: "0.1.0".into(),
            artifact_sha256: "a".repeat(64),
            source: None,
            extensions: BTreeMap::new(),
        }],
        document: serde_json::json!({"nodes": [{"id": "button-1"}]}),
        assets: vec![],
        private_section: Some(serde_json::json!({"credentialRefs": ["mail-primary"]})),
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::from([(
            "futureRevisionField".into(),
            serde_json::json!({"preserve": true}),
        )]),
    })
}

fn snapshot(revision: &WorkspaceRevision) -> Result<SnapshotRef, Box<dyn Error>> {
    Ok(SnapshotRef {
        schema_version: 1,
        snapshot_id: SnapshotId::try_new("snapshot-before-redesign")?,
        workspace_id: revision.workspace_id.clone(),
        revision_id: revision.revision_id.clone(),
        name: "Before redesign".into(),
        comment: Some("Known-good layout".into()),
        created_at_unix_ms: 1_797_000_000_000,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::from([(
            "futureSnapshotField".into(),
            serde_json::json!({"preserve": true}),
        )]),
    })
}

#[test]
fn named_snapshot_is_immutable_and_restores_a_new_draft() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let revision = revision()?;
    store.save_revision(&revision)?;
    let snapshot = snapshot(&revision)?;

    let first = store.save_snapshot(&snapshot)?;
    assert_eq!(first.checksum_sha256.len(), 64);
    assert_eq!(store.save_snapshot(&snapshot)?, first);
    assert_eq!(
        store.load_snapshot(&snapshot.workspace_id, &snapshot.snapshot_id)?,
        snapshot
    );

    let mut conflicting = snapshot.clone();
    conflicting.comment = Some("Changed in place".into());
    assert!(matches!(
        store.save_snapshot(&conflicting),
        Err(ProfileStoreError::ImmutableSnapshotConflict)
    ));

    let restored = store.create_draft_from_snapshot(
        &snapshot.workspace_id,
        &snapshot.snapshot_id,
        DraftId::try_new("draft-restored")?,
    )?;
    assert_eq!(restored.draft.version.get(), 1);
    assert_eq!(restored.draft.base_revision_id, Some(revision.revision_id));
    assert_eq!(restored.draft.document, revision.document);
    assert_eq!(restored.draft.package_lock, revision.package_lock);
    assert_eq!(restored.draft.unknown_fields, revision.unknown_fields);
    Ok(())
}

#[test]
fn snapshot_requires_an_existing_revision_and_verifies_checksum() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let revision = revision()?;
    let snapshot = snapshot(&revision)?;
    assert!(matches!(
        store.save_snapshot(&snapshot),
        Err(ProfileStoreError::NotFound)
    ));

    store.save_revision(&revision)?;
    store.save_snapshot(&snapshot)?;
    let workspace_key = format!("{:x}", Sha256::digest(snapshot.workspace_id.as_str()));
    let snapshot_key = format!("{:x}", Sha256::digest(snapshot.snapshot_id.as_str()));
    let path = directory
        .path()
        .join("workspaces")
        .join(workspace_key)
        .join("snapshots")
        .join(format!("{snapshot_key}.json"));
    let mut stored: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    stored["checksumSha256"] = serde_json::Value::String("0".repeat(64));
    fs::write(path, serde_json::to_vec(&stored)?)?;
    assert!(matches!(
        store.load_snapshot(&snapshot.workspace_id, &snapshot.snapshot_id),
        Err(ProfileStoreError::ChecksumMismatch)
    ));
    Ok(())
}
