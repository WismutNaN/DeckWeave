// SPDX-License-Identifier: GPL-3.0-or-later
//! Immutable workspace revisions and content-addressed profile assets.

use deckweave_application::{AtomicFileStore, StorageError};
use deckweave_domain::ids::{AssetId, SnapshotId, WorkspaceId, WorkspaceRevisionId};
pub use deckweave_workspace::{
    AssetReference, DraftCommand, MutationKey, PackageLockEntry, WorkspaceDraft, WorkspaceRevision,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use thiserror::Error;

pub mod draft;
pub use draft::{DraftEditor, DraftMutationError, LoadedDraft, MutationOutcome, MutationReceipt};
pub mod export;
pub use export::{
    ExportError, ExportSelection, ImportInspection, ImportResult, ImportRevisionSummary,
    RevisionExportSelection,
};
pub mod snapshot;
pub use snapshot::{SnapshotMetadata, SnapshotRef};

const STORAGE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDescriptor {
    pub storage_version: u32,
    pub id: AssetId,
    pub sha256: String,
    pub media_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionMetadata {
    pub workspace_id: WorkspaceId,
    pub revision_id: WorkspaceRevisionId,
    pub checksum_sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredRevision {
    storage_version: u32,
    checksum_sha256: String,
    revision: WorkspaceRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileStoreLimits {
    pub maximum_revision_bytes: u64,
    pub maximum_asset_bytes: u64,
    pub maximum_draft_transactions: usize,
    pub maximum_export_bytes: u64,
}

impl Default for ProfileStoreLimits {
    fn default() -> Self {
        Self {
            maximum_revision_bytes: 16 * 1024 * 1024,
            maximum_asset_bytes: 128 * 1024 * 1024,
            maximum_draft_transactions: 10_000,
            maximum_export_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProfileStoreError {
    #[error("profile storage I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("profile document JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("profile document is invalid: {0}")]
    InvalidDocument(String),
    #[error("stored object is not found")]
    NotFound,
    #[error("stored object exceeds configured limit")]
    TooLarge,
    #[error("stored checksum does not match bytes")]
    ChecksumMismatch,
    #[error("revision ID already belongs to different immutable bytes")]
    ImmutableRevisionConflict,
    #[error("snapshot ID already belongs to different immutable bytes")]
    ImmutableSnapshotConflict,
    #[error("asset metadata conflicts with existing content-addressed object")]
    AssetMetadataConflict,
    #[error("profile store mutex is poisoned")]
    LockPoisoned,
}

pub trait ProfileStore: Send + Sync {
    fn save_revision(
        &self,
        revision: &WorkspaceRevision,
    ) -> Result<RevisionMetadata, ProfileStoreError>;
    fn load_revision(
        &self,
        workspace_id: &WorkspaceId,
        revision_id: &WorkspaceRevisionId,
    ) -> Result<WorkspaceRevision, ProfileStoreError>;
    fn put_asset(
        &self,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<AssetDescriptor, ProfileStoreError>;
    fn read_asset(
        &self,
        asset_id: &AssetId,
    ) -> Result<(AssetDescriptor, Vec<u8>), ProfileStoreError>;
    fn save_snapshot(&self, snapshot: &SnapshotRef) -> Result<SnapshotMetadata, ProfileStoreError>;
    fn load_snapshot(
        &self,
        workspace_id: &WorkspaceId,
        snapshot_id: &SnapshotId,
    ) -> Result<SnapshotRef, ProfileStoreError>;
}

pub struct JsonProfileStore {
    root: PathBuf,
    atomic_files: Arc<dyn AtomicFileStore>,
    limits: ProfileStoreLimits,
    mutation_lock: Mutex<()>,
}

impl JsonProfileStore {
    #[must_use]
    pub fn new(
        root: PathBuf,
        atomic_files: Arc<dyn AtomicFileStore>,
        limits: ProfileStoreLimits,
    ) -> Self {
        Self {
            root,
            atomic_files,
            limits,
            mutation_lock: Mutex::new(()),
        }
    }

    fn revision_path(
        &self,
        workspace_id: &WorkspaceId,
        revision_id: &WorkspaceRevisionId,
    ) -> PathBuf {
        self.root
            .join("workspaces")
            .join(opaque_key(workspace_id.as_str()))
            .join("revisions")
            .join(format!("{}.json", opaque_key(revision_id.as_str())))
    }

    fn asset_paths(&self, asset_id: &AssetId) -> (PathBuf, PathBuf) {
        let prefix = &asset_id.as_str()[..2];
        let directory = self.root.join("assets/sha256").join(prefix);
        (
            directory.join(format!("{}.bin", asset_id.as_str())),
            directory.join(format!("{}.json", asset_id.as_str())),
        )
    }
}

impl ProfileStore for JsonProfileStore {
    fn save_revision(
        &self,
        revision: &WorkspaceRevision,
    ) -> Result<RevisionMetadata, ProfileStoreError> {
        validate_revision(revision)?;
        let revision_bytes = canonical_json(revision)?;
        if u64::try_from(revision_bytes.len()).unwrap_or(u64::MAX)
            > self.limits.maximum_revision_bytes
        {
            return Err(ProfileStoreError::TooLarge);
        }
        let checksum_sha256 = digest(&revision_bytes);
        let record = StoredRevision {
            storage_version: STORAGE_VERSION,
            checksum_sha256: checksum_sha256.clone(),
            revision: revision.clone(),
        };
        let record_bytes = canonical_json(&record)?;
        let path = self.revision_path(&revision.workspace_id, &revision.revision_id);
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        if path.exists() {
            let existing = self.load_revision(&revision.workspace_id, &revision.revision_id)?;
            if canonical_json(&existing)? == revision_bytes {
                return Ok(RevisionMetadata {
                    workspace_id: revision.workspace_id.clone(),
                    revision_id: revision.revision_id.clone(),
                    checksum_sha256,
                    size_bytes: u64::try_from(revision_bytes.len()).unwrap_or(u64::MAX),
                });
            }
            return Err(ProfileStoreError::ImmutableRevisionConflict);
        }
        self.atomic_files.write_atomic(&path, &record_bytes, None)?;
        let stored = self.load_revision(&revision.workspace_id, &revision.revision_id)?;
        if canonical_json(&stored)? != revision_bytes {
            return Err(ProfileStoreError::ChecksumMismatch);
        }
        Ok(RevisionMetadata {
            workspace_id: revision.workspace_id.clone(),
            revision_id: revision.revision_id.clone(),
            checksum_sha256,
            size_bytes: u64::try_from(revision_bytes.len()).unwrap_or(u64::MAX),
        })
    }

    fn load_revision(
        &self,
        workspace_id: &WorkspaceId,
        revision_id: &WorkspaceRevisionId,
    ) -> Result<WorkspaceRevision, ProfileStoreError> {
        let path = self.revision_path(workspace_id, revision_id);
        let bytes = read_bounded(&path, self.limits.maximum_revision_bytes)?;
        let record: StoredRevision = serde_json::from_slice(&bytes)?;
        if record.storage_version != STORAGE_VERSION {
            return Err(ProfileStoreError::InvalidDocument(format!(
                "unsupported storage version {}",
                record.storage_version
            )));
        }
        validate_revision(&record.revision)?;
        if record.revision.workspace_id != *workspace_id
            || record.revision.revision_id != *revision_id
        {
            return Err(ProfileStoreError::InvalidDocument(
                "record identity does not match its lookup key".into(),
            ));
        }
        let revision_bytes = canonical_json(&record.revision)?;
        if digest(&revision_bytes) != record.checksum_sha256 {
            return Err(ProfileStoreError::ChecksumMismatch);
        }
        Ok(record.revision)
    }

    fn put_asset(
        &self,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<AssetDescriptor, ProfileStoreError> {
        if media_type.is_empty() || media_type.len() > 256 {
            return Err(ProfileStoreError::InvalidDocument(
                "asset media type is empty or too long".into(),
            ));
        }
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.limits.maximum_asset_bytes {
            return Err(ProfileStoreError::TooLarge);
        }
        let sha256 = digest(bytes);
        let asset_id = AssetId::try_new(&sha256)
            .map_err(|error| ProfileStoreError::InvalidDocument(error.to_string()))?;
        let descriptor = AssetDescriptor {
            storage_version: STORAGE_VERSION,
            id: asset_id.clone(),
            sha256,
            media_type: media_type.into(),
            size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        };
        let (data_path, metadata_path) = self.asset_paths(&asset_id);
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;

        if data_path.exists() {
            let existing = read_bounded(&data_path, self.limits.maximum_asset_bytes)?;
            if digest(&existing) != asset_id.as_str() {
                return Err(ProfileStoreError::ChecksumMismatch);
            }
        } else {
            self.atomic_files.write_atomic(&data_path, bytes, None)?;
        }
        if metadata_path.exists() {
            let (existing, existing_bytes) = self.read_asset(&asset_id)?;
            if existing != descriptor || existing_bytes != bytes {
                return Err(ProfileStoreError::AssetMetadataConflict);
            }
            return Ok(existing);
        }
        self.atomic_files
            .write_atomic(&metadata_path, &canonical_json(&descriptor)?, None)?;
        let (stored, stored_bytes) = self.read_asset(&asset_id)?;
        if stored != descriptor || stored_bytes != bytes {
            return Err(ProfileStoreError::ChecksumMismatch);
        }
        Ok(stored)
    }

    fn read_asset(
        &self,
        asset_id: &AssetId,
    ) -> Result<(AssetDescriptor, Vec<u8>), ProfileStoreError> {
        let (data_path, metadata_path) = self.asset_paths(asset_id);
        let metadata_bytes = read_bounded(&metadata_path, 1024 * 1024)?;
        let descriptor: AssetDescriptor = serde_json::from_slice(&metadata_bytes)?;
        if descriptor.storage_version != STORAGE_VERSION || descriptor.id != *asset_id {
            return Err(ProfileStoreError::InvalidDocument(
                "asset metadata identity/version mismatch".into(),
            ));
        }
        let bytes = read_bounded(&data_path, self.limits.maximum_asset_bytes)?;
        if descriptor.size_bytes != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            || descriptor.sha256 != asset_id.as_str()
            || digest(&bytes) != asset_id.as_str()
        {
            return Err(ProfileStoreError::ChecksumMismatch);
        }
        Ok((descriptor, bytes))
    }

    fn save_snapshot(&self, snapshot: &SnapshotRef) -> Result<SnapshotMetadata, ProfileStoreError> {
        self.save_snapshot_inner(snapshot)
    }

    fn load_snapshot(
        &self,
        workspace_id: &WorkspaceId,
        snapshot_id: &SnapshotId,
    ) -> Result<SnapshotRef, ProfileStoreError> {
        self.load_snapshot_inner(workspace_id, snapshot_id)
    }
}

fn validate_revision(revision: &WorkspaceRevision) -> Result<(), ProfileStoreError> {
    deckweave_workspace::validate_revision(revision)
        .map_err(|error| ProfileStoreError::InvalidDocument(error.to_string()))
}

fn validate_workspace_content(
    package_lock: &[PackageLockEntry],
    document: &serde_json::Value,
    assets: &[AssetReference],
) -> Result<(), ProfileStoreError> {
    deckweave_workspace::validate_workspace_content(package_lock, document, assets)
        .map_err(|error| ProfileStoreError::InvalidDocument(error.to_string()))
}

fn canonical_json(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn opaque_key(value: &str) -> String {
    digest(value.as_bytes())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, ProfileStoreError> {
    let metadata = fs::metadata(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            ProfileStoreError::NotFound
        } else {
            ProfileStoreError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    if metadata.len() > maximum {
        return Err(ProfileStoreError::TooLarge);
    }
    fs::read(path).map_err(|source| ProfileStoreError::Io {
        path: path.to_path_buf(),
        source,
    })
}
