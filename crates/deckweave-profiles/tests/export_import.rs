// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{CredentialRef, DraftId, PackageId, WorkspaceId, WorkspaceRevisionId};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{
    AssetReference, ExportError, ExportSelection, JsonProfileStore, PackageLockEntry, ProfileStore,
    ProfileStoreLimits, RevisionExportSelection, WorkspaceRevision,
};
use std::{collections::BTreeMap, error::Error, path::Path, sync::Arc};

fn native_store(root: &Path) -> JsonProfileStore {
    JsonProfileStore::new(
        root.to_path_buf(),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    )
}

fn revision_with_asset(store: &JsonProfileStore) -> Result<WorkspaceRevision, Box<dyn Error>> {
    let descriptor = store.put_asset(b"portable icon", "image/png")?;
    Ok(WorkspaceRevision {
        schema_version: 1,
        workspace_id: WorkspaceId::try_new("workspace-portable")?,
        revision_id: WorkspaceRevisionId::try_new("revision-exported")?,
        package_lock: vec![PackageLockEntry {
            package_id: PackageId::try_new("org.deckweave.fixture.virtual")?,
            version: "0.1.0".into(),
            artifact_sha256: "b".repeat(64),
            source: Some("https://example.invalid/metadata-only".into()),
            extensions: BTreeMap::new(),
        }],
        document: serde_json::json!({"nodes": [{"icon": descriptor.id.as_str()}]}),
        assets: vec![AssetReference {
            id: descriptor.id,
            sha256: descriptor.sha256,
            media_type: descriptor.media_type,
            size_bytes: descriptor.size_bytes,
            extensions: BTreeMap::new(),
        }],
        private_section: Some(serde_json::json!({
            "credentialRefs": ["mail-primary"],
            "accountLabel": "Work mail"
        })),
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::from([(
            "futurePortableField".into(),
            serde_json::json!({"preserve": true}),
        )]),
    })
}

fn selection(revision: &WorkspaceRevision, assets: bool, private: bool) -> ExportSelection {
    ExportSelection {
        revisions: vec![RevisionExportSelection {
            workspace_id: revision.workspace_id.clone(),
            revision_id: revision.revision_id.clone(),
        }],
        include_assets: assets,
        include_private_sections: private,
    }
}

#[test]
fn selective_export_imports_assets_and_preserves_unknown_data() -> Result<(), Box<dyn Error>> {
    let source_directory = tempfile::tempdir()?;
    let source = native_store(source_directory.path());
    let revision = revision_with_asset(&source)?;
    source.save_revision(&revision)?;
    let bytes = source.export_revisions(&selection(&revision, true, false))?;

    let target_directory = tempfile::tempdir()?;
    let target = native_store(target_directory.path());
    let inspection = target.inspect_import(&bytes)?;
    assert_eq!(inspection.revisions.len(), 1);
    assert_eq!(
        inspection.included_assets,
        vec![revision.assets[0].id.clone()]
    );
    assert!(inspection.missing_assets.is_empty());
    assert!(inspection.unresolved_credential_refs.is_empty());
    assert!(!inspection.revisions[0].has_private_section);

    let imported = target.import_revision_as_draft(
        &bytes,
        &revision.workspace_id,
        &revision.revision_id,
        DraftId::try_new("draft-from-export")?,
        false,
    )?;
    assert_eq!(
        imported.installed_assets,
        vec![revision.assets[0].id.clone()]
    );
    assert!(imported.draft.draft.private_section.is_none());
    assert_eq!(imported.draft.draft.unknown_fields, revision.unknown_fields);
    assert_eq!(imported.draft.draft.package_lock, revision.package_lock);
    assert_eq!(
        target.read_asset(&revision.assets[0].id)?.1,
        b"portable icon"
    );
    Ok(())
}

#[test]
fn metadata_only_export_reports_missing_assets_and_unresolved_credentials()
-> Result<(), Box<dyn Error>> {
    let source_directory = tempfile::tempdir()?;
    let source = native_store(source_directory.path());
    let revision = revision_with_asset(&source)?;
    source.save_revision(&revision)?;
    let bytes = source.export_revisions(&selection(&revision, false, true))?;

    let target_directory = tempfile::tempdir()?;
    let target = native_store(target_directory.path());
    let inspection = target.inspect_import(&bytes)?;
    assert_eq!(
        inspection.missing_assets,
        vec![revision.assets[0].id.clone()]
    );
    assert_eq!(
        inspection.unresolved_credential_refs,
        vec![CredentialRef::try_new("mail-primary")?]
    );
    assert!(matches!(
        target.import_revision_as_draft(
            &bytes,
            &revision.workspace_id,
            &revision.revision_id,
            DraftId::try_new("draft-reject-missing")?,
            false,
        ),
        Err(ExportError::MissingAssets)
    ));

    let imported = target.import_revision_as_draft(
        &bytes,
        &revision.workspace_id,
        &revision.revision_id,
        DraftId::try_new("draft-allow-missing")?,
        true,
    )?;
    assert!(imported.installed_assets.is_empty());
    assert_eq!(
        imported.unresolved_credential_refs,
        vec![CredentialRef::try_new("mail-primary")?]
    );
    assert_eq!(
        imported.draft.draft.private_section,
        revision.private_section
    );
    Ok(())
}

#[test]
fn tampered_bundle_is_rejected_before_any_import_write() -> Result<(), Box<dyn Error>> {
    let source_directory = tempfile::tempdir()?;
    let source = native_store(source_directory.path());
    let revision = revision_with_asset(&source)?;
    source.save_revision(&revision)?;
    let bytes = source.export_revisions(&selection(&revision, true, true))?;
    let mut tampered: serde_json::Value = serde_json::from_slice(&bytes)?;
    tampered["bundle"]["revisions"][0]["document"]["nodes"][0]["icon"] =
        serde_json::json!("tampered");
    let bytes = serde_json::to_vec(&tampered)?;

    let target_directory = tempfile::tempdir()?;
    let target = native_store(target_directory.path());
    assert!(matches!(
        target.inspect_import(&bytes),
        Err(ExportError::InvalidBundle(_))
    ));
    Ok(())
}
