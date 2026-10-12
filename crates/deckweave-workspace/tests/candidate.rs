// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{AssetId, DraftId, DraftVersion, PackageId, WorkspaceId};
use deckweave_workspace::{
    AssetReference, PackageLockEntry, WorkspaceCandidate, WorkspaceDraft, WorkspaceValidationError,
    validate_draft,
};
use std::{collections::BTreeMap, error::Error};

fn draft() -> Result<WorkspaceDraft, Box<dyn Error>> {
    Ok(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("candidate-draft")?,
        workspace_id: WorkspaceId::try_new("candidate-workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
}

fn package() -> Result<PackageLockEntry, Box<dyn Error>> {
    Ok(PackageLockEntry {
        package_id: PackageId::try_new("org.deckweave.virtual")?,
        version: "1.0.0".into(),
        artifact_sha256: "a".repeat(64),
        source: None,
        extensions: BTreeMap::new(),
    })
}

fn asset() -> Result<AssetReference, Box<dyn Error>> {
    let sha256 = "b".repeat(64);
    Ok(AssetReference {
        id: AssetId::try_new(&sha256)?,
        sha256,
        media_type: "image/png".into(),
        size_bytes: 12,
        extensions: BTreeMap::new(),
    })
}

#[test]
fn package_and_asset_identities_are_unique_within_a_candidate() -> Result<(), Box<dyn Error>> {
    let mut duplicate_package = draft()?;
    let package = package()?;
    duplicate_package.package_lock = vec![package.clone(), package];
    assert_eq!(
        validate_draft(&duplicate_package),
        Err(WorkspaceValidationError::DuplicatePackageLock)
    );

    let mut duplicate_asset = draft()?;
    let asset = asset()?;
    duplicate_asset.assets = vec![asset.clone(), asset];
    assert_eq!(
        validate_draft(&duplicate_asset),
        Err(WorkspaceValidationError::DuplicateAssetReference)
    );
    Ok(())
}

#[test]
fn candidate_hashes_bind_full_content_lock_and_assets() -> Result<(), Box<dyn Error>> {
    let original = WorkspaceCandidate::from_draft(draft()?)?;

    let mut with_document = draft()?;
    with_document.document = serde_json::json!({"fixture": {"kind": "virtual", "routes": []}});
    let with_document = WorkspaceCandidate::from_draft(with_document)?;
    assert_ne!(original.content_sha256(), with_document.content_sha256());
    assert_eq!(
        original.package_lock_sha256(),
        with_document.package_lock_sha256()
    );

    let mut with_package = draft()?;
    with_package.package_lock.push(package()?);
    let with_package = WorkspaceCandidate::from_draft(with_package)?;
    assert_ne!(
        original.package_lock_sha256(),
        with_package.package_lock_sha256()
    );
    assert_ne!(original.content_sha256(), with_package.content_sha256());

    let mut with_asset = draft()?;
    with_asset.assets.push(asset()?);
    let with_asset = WorkspaceCandidate::from_draft(with_asset)?;
    assert_ne!(original.assets_sha256(), with_asset.assets_sha256());
    assert_ne!(original.content_sha256(), with_asset.content_sha256());
    Ok(())
}
