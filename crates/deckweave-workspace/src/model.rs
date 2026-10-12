// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{
    AssetId, DraftId, DraftVersion, PackageId, WorkspaceId, WorkspaceRevisionId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DraftCommand {
    Set {
        pointer: String,
        value: serde_json::Value,
    },
    Remove {
        pointer: String,
    },
    ReplaceDocument {
        document: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageLockEntry {
    pub package_id: PackageId,
    pub version: String,
    pub artifact_sha256: String,
    pub source: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetReference {
    pub id: AssetId,
    pub sha256: String,
    pub media_type: String,
    pub size_bytes: u64,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRevision {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub revision_id: WorkspaceRevisionId,
    pub package_lock: Vec<PackageLockEntry>,
    pub document: serde_json::Value,
    #[serde(default)]
    pub assets: Vec<AssetReference>,
    pub private_section: Option<serde_json::Value>,
    #[serde(default)]
    pub extensions: BTreeMap<String, serde_json::Value>,
    #[serde(flatten)]
    pub unknown_fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDraft {
    pub schema_version: u32,
    pub draft_id: DraftId,
    pub workspace_id: WorkspaceId,
    pub base_revision_id: Option<WorkspaceRevisionId>,
    pub version: DraftVersion,
    pub package_lock: Vec<PackageLockEntry>,
    pub document: serde_json::Value,
    #[serde(default)]
    pub assets: Vec<AssetReference>,
    pub private_section: Option<serde_json::Value>,
    #[serde(default)]
    pub extensions: BTreeMap<String, serde_json::Value>,
    #[serde(flatten)]
    pub unknown_fields: BTreeMap<String, serde_json::Value>,
}
