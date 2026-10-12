// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{SnapshotId, WorkspaceId, WorkspaceRevisionId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

const MAXIMUM_SNAPSHOT_NAME_BYTES: usize = 128;
const MAXIMUM_SNAPSHOT_COMMENT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRef {
    pub schema_version: u32,
    pub snapshot_id: SnapshotId,
    pub workspace_id: WorkspaceId,
    pub revision_id: WorkspaceRevisionId,
    pub name: String,
    pub comment: Option<String>,
    pub created_at_unix_ms: u64,
    #[serde(default)]
    pub extensions: BTreeMap<String, serde_json::Value>,
    #[serde(flatten)]
    pub unknown_fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SnapshotValidationError {
    #[error("unsupported snapshot schema version")]
    UnsupportedSchema,
    #[error("snapshot name must be non-empty and at most 128 bytes")]
    InvalidName,
    #[error("snapshot comment exceeds 16 KiB")]
    CommentTooLarge,
}

pub fn validate_snapshot(snapshot: &SnapshotRef) -> Result<(), SnapshotValidationError> {
    if snapshot.schema_version != 1 {
        return Err(SnapshotValidationError::UnsupportedSchema);
    }
    if snapshot.name.trim().is_empty() || snapshot.name.len() > MAXIMUM_SNAPSHOT_NAME_BYTES {
        return Err(SnapshotValidationError::InvalidName);
    }
    if snapshot
        .comment
        .as_ref()
        .is_some_and(|comment| comment.len() > MAXIMUM_SNAPSHOT_COMMENT_BYTES)
    {
        return Err(SnapshotValidationError::CommentTooLarge);
    }
    Ok(())
}
