// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{AssetReference, PackageLockEntry, WorkspaceDraft, WorkspaceRevision};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkspaceValidationError {
    #[error("unsupported workspace schema version")]
    UnsupportedSchema,
    #[error("workspace document must be a JSON object")]
    DocumentIsNotObject,
    #[error("invalid package lock entry")]
    InvalidPackageLock,
    #[error("package lock contains duplicate package IDs")]
    DuplicatePackageLock,
    #[error("invalid asset reference")]
    InvalidAssetReference,
    #[error("workspace contains duplicate asset IDs")]
    DuplicateAssetReference,
}

pub fn validate_revision(revision: &WorkspaceRevision) -> Result<(), WorkspaceValidationError> {
    if revision.schema_version != 1 {
        return Err(WorkspaceValidationError::UnsupportedSchema);
    }
    validate_workspace_content(&revision.package_lock, &revision.document, &revision.assets)
}

pub fn validate_draft(draft: &WorkspaceDraft) -> Result<(), WorkspaceValidationError> {
    if draft.schema_version != 1 {
        return Err(WorkspaceValidationError::UnsupportedSchema);
    }
    validate_workspace_content(&draft.package_lock, &draft.document, &draft.assets)
}

pub fn validate_workspace_content(
    package_lock: &[PackageLockEntry],
    document: &serde_json::Value,
    assets: &[AssetReference],
) -> Result<(), WorkspaceValidationError> {
    if !document.is_object() {
        return Err(WorkspaceValidationError::DocumentIsNotObject);
    }
    let mut package_ids = BTreeSet::new();
    for package in package_lock {
        if package.version.is_empty()
            || package.version.len() > 128
            || !is_sha256(&package.artifact_sha256)
        {
            return Err(WorkspaceValidationError::InvalidPackageLock);
        }
        if !package_ids.insert(&package.package_id) {
            return Err(WorkspaceValidationError::DuplicatePackageLock);
        }
    }
    let mut asset_ids = BTreeSet::new();
    for asset in assets {
        if asset.id.as_str() != asset.sha256.to_ascii_lowercase()
            || !is_sha256(&asset.sha256)
            || asset.media_type.is_empty()
            || asset.media_type.len() > 256
        {
            return Err(WorkspaceValidationError::InvalidAssetReference);
        }
        if !asset_ids.insert(&asset.id) {
            return Err(WorkspaceValidationError::DuplicateAssetReference);
        }
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
