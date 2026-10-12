// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{PackageId, WorkspaceId, WorkspaceRevisionId};
use deckweave_platform::{AtomicFileStore, PublicationError, StorageError};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{
    JsonProfileStore, PackageLockEntry, ProfileStore, ProfileStoreError, ProfileStoreLimits,
    WorkspaceRevision,
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

fn revision(revision_id: &str) -> Result<WorkspaceRevision, Box<dyn Error>> {
    Ok(WorkspaceRevision {
        schema_version: 1,
        workspace_id: WorkspaceId::try_new("workspace-main")?,
        revision_id: WorkspaceRevisionId::try_new(revision_id)?,
        package_lock: vec![PackageLockEntry {
            package_id: PackageId::try_new("org.deckweave.fixture.virtual")?,
            version: "0.1.0".into(),
            artifact_sha256: "a".repeat(64),
            source: Some("https://example.invalid/plugin".into()),
            extensions: BTreeMap::new(),
        }],
        document: serde_json::json!({ "bindings": [], "layout": { "zoom": 0.75 } }),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::from([("org.deckweave.test".into(), serde_json::json!({"x": 1}))]),
        unknown_fields: BTreeMap::from([(
            "futureTopLevel".into(),
            serde_json::json!({"mustSurvive": true}),
        )]),
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
fn revision_is_immutable_checksum_verified_and_keeps_unknown_fields() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let original = revision("revision-1")?;
    let first = store.save_revision(&original)?;
    assert_eq!(first.checksum_sha256.len(), 64);
    assert_eq!(store.save_revision(&original)?, first);
    assert_eq!(
        store.load_revision(&original.workspace_id, &original.revision_id)?,
        original
    );

    let mut conflicting = original.clone();
    conflicting.document = serde_json::json!({"bindings": ["different"]});
    assert!(matches!(
        store.save_revision(&conflicting),
        Err(ProfileStoreError::ImmutableRevisionConflict)
    ));

    let workspace_key = format!("{:x}", Sha256::digest(original.workspace_id.as_str()));
    let revision_key = format!("{:x}", Sha256::digest(original.revision_id.as_str()));
    let record_path = directory
        .path()
        .join("workspaces")
        .join(workspace_key)
        .join("revisions")
        .join(format!("{revision_key}.json"));
    let mut record: serde_json::Value = serde_json::from_slice(&fs::read(&record_path)?)?;
    record["checksumSha256"] = serde_json::Value::String("0".repeat(64));
    fs::write(&record_path, serde_json::to_vec_pretty(&record)?)?;
    assert!(matches!(
        store.load_revision(&original.workspace_id, &original.revision_id),
        Err(ProfileStoreError::ChecksumMismatch)
    ));
    Ok(())
}

#[test]
fn content_addressed_assets_deduplicate_and_detect_corruption() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = native_store(directory.path());
    let bytes = b"deterministic image bytes";
    let first = store.put_asset(bytes, "image/png")?;
    let second = store.put_asset(bytes, "image/png")?;
    assert_eq!(first, second);
    assert_eq!(store.read_asset(&first.id)?.1, bytes);

    let data_path = directory
        .path()
        .join("assets/sha256")
        .join(&first.id.as_str()[..2])
        .join(format!("{}.bin", first.id));
    fs::write(data_path, b"corrupt")?;
    assert!(matches!(
        store.read_asset(&first.id),
        Err(ProfileStoreError::ChecksumMismatch)
    ));
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
        _backup: Option<&Path>,
    ) -> Result<(), PublicationError> {
        let call = self.call.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(PublicationError::not_published(StorageError::Unavailable(
                "injected disk failure".into(),
            )));
        }
        let parent = target
            .parent()
            .ok_or_else(|| StorageError::Unavailable("missing parent".into()))
            .map_err(PublicationError::not_published)?;
        fs::create_dir_all(parent).map_err(PublicationError::not_published)?;
        fs::write(target, bytes).map_err(PublicationError::not_published)?;
        Ok(())
    }
}

#[test]
fn failed_asset_metadata_publish_leaves_only_an_unreachable_orphan() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let failing = JsonProfileStore::new(
        directory.path().to_path_buf(),
        Arc::new(FailOnCall {
            call: AtomicUsize::new(0),
            fail_on: 2,
        }),
        ProfileStoreLimits::default(),
    );
    let bytes = b"asset whose metadata publication fails";
    assert!(matches!(
        failing.put_asset(bytes, "application/octet-stream"),
        Err(ProfileStoreError::Storage(_))
    ));

    let id = deckweave_domain::ids::AssetId::try_new(format!("{:x}", Sha256::digest(bytes)))?;
    assert!(matches!(
        failing.read_asset(&id),
        Err(ProfileStoreError::NotFound)
    ));

    let recovered = native_store(directory.path());
    let descriptor = recovered.put_asset(bytes, "application/octet-stream")?;
    assert_eq!(descriptor.id, id);
    assert_eq!(recovered.read_asset(&id)?.1, bytes);
    Ok(())
}
