// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_domain::ids::{DraftId, DraftVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotMetadata {
    pub workspace_id: WorkspaceId,
    pub snapshot_id: SnapshotId,
    pub revision_id: WorkspaceRevisionId,
    pub checksum_sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSnapshot {
    storage_version: u32,
    checksum_sha256: String,
    snapshot: SnapshotRef,
}

impl JsonProfileStore {
    fn snapshot_path(&self, workspace_id: &WorkspaceId, snapshot_id: &SnapshotId) -> PathBuf {
        self.root
            .join("workspaces")
            .join(opaque_key(workspace_id.as_str()))
            .join("snapshots")
            .join(format!("{}.json", opaque_key(snapshot_id.as_str())))
    }

    pub fn create_draft_from_snapshot(
        &self,
        workspace_id: &WorkspaceId,
        snapshot_id: &SnapshotId,
        draft_id: DraftId,
    ) -> Result<LoadedDraft, DraftMutationError> {
        let snapshot = self.load_snapshot(workspace_id, snapshot_id)?;
        let revision = self.load_revision(workspace_id, &snapshot.revision_id)?;
        self.create_draft(&WorkspaceDraft {
            schema_version: revision.schema_version,
            draft_id,
            workspace_id: revision.workspace_id,
            base_revision_id: Some(revision.revision_id),
            version: DraftVersion::try_new(1).map_err(|error| {
                ProfileStoreError::InvalidDocument(format!(
                    "invalid initial draft version: {error}"
                ))
            })?,
            package_lock: revision.package_lock,
            document: revision.document,
            assets: revision.assets,
            private_section: revision.private_section,
            extensions: revision.extensions,
            unknown_fields: revision.unknown_fields,
        })
    }
}

impl JsonProfileStore {
    pub(crate) fn save_snapshot_inner(
        &self,
        snapshot: &SnapshotRef,
    ) -> Result<SnapshotMetadata, ProfileStoreError> {
        validate_snapshot(snapshot)?;
        let revision = self.load_revision(&snapshot.workspace_id, &snapshot.revision_id)?;
        if revision.workspace_id != snapshot.workspace_id {
            return Err(ProfileStoreError::InvalidDocument(
                "snapshot revision belongs to another workspace".into(),
            ));
        }

        let snapshot_bytes = canonical_json(snapshot)?;
        if u64::try_from(snapshot_bytes.len()).unwrap_or(u64::MAX)
            > self.limits.maximum_revision_bytes
        {
            return Err(ProfileStoreError::TooLarge);
        }
        let checksum_sha256 = digest(&snapshot_bytes);
        let stored = StoredSnapshot {
            storage_version: STORAGE_VERSION,
            checksum_sha256: checksum_sha256.clone(),
            snapshot: snapshot.clone(),
        };
        let stored_bytes = canonical_json(&stored)?;
        let path = self.snapshot_path(&snapshot.workspace_id, &snapshot.snapshot_id);
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        if path.exists() {
            let existing =
                self.load_snapshot_inner(&snapshot.workspace_id, &snapshot.snapshot_id)?;
            if existing != *snapshot {
                return Err(ProfileStoreError::ImmutableSnapshotConflict);
            }
        } else {
            self.atomic_files.write_atomic(&path, &stored_bytes, None)?;
        }
        Ok(SnapshotMetadata {
            workspace_id: snapshot.workspace_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            revision_id: snapshot.revision_id.clone(),
            checksum_sha256,
            size_bytes: u64::try_from(stored_bytes.len()).unwrap_or(u64::MAX),
        })
    }

    pub(crate) fn load_snapshot_inner(
        &self,
        workspace_id: &WorkspaceId,
        snapshot_id: &SnapshotId,
    ) -> Result<SnapshotRef, ProfileStoreError> {
        let path = self.snapshot_path(workspace_id, snapshot_id);
        let bytes = read_bounded(&path, self.limits.maximum_revision_bytes)?;
        let stored: StoredSnapshot = serde_json::from_slice(&bytes)?;
        if stored.storage_version != STORAGE_VERSION
            || stored.snapshot.workspace_id != *workspace_id
            || stored.snapshot.snapshot_id != *snapshot_id
        {
            return Err(ProfileStoreError::InvalidDocument(
                "snapshot identity or storage version does not match".into(),
            ));
        }
        validate_snapshot(&stored.snapshot)?;
        if digest(&canonical_json(&stored.snapshot)?) != stored.checksum_sha256 {
            return Err(ProfileStoreError::ChecksumMismatch);
        }
        Ok(stored.snapshot)
    }
}

fn validate_snapshot(snapshot: &SnapshotRef) -> Result<(), ProfileStoreError> {
    deckweave_workspace::validate_snapshot(snapshot)
        .map_err(|error| ProfileStoreError::InvalidDocument(error.to_string()))
}
