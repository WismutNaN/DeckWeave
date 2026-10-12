// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
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
