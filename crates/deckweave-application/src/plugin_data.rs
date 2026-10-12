// SPDX-License-Identifier: GPL-3.0-or-later
//! Scoped provider data. Implementations receive authority from the host, not from JSON payloads.
use deckweave_domain::{
    ids::{PackageId, PluginInstanceId},
    schema::SchemaRef,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDataScope {
    pub package_id: PackageId,
    pub instance_id: PluginInstanceId,
    pub data_root_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDocument {
    pub document_id: String,
    pub revision: u64,
    pub schema: SchemaRef,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginDataError {
    #[error("plugin data authorization denied")]
    Denied,
    #[error("document revision conflict")]
    Conflict,
    #[error("plugin data quota exceeded")]
    Quota,
    #[error("invalid plugin data")]
    Invalid,
    #[error("plugin data unavailable: {0}")]
    Unavailable(String),
}

/// A handle is bound to one authenticated scope. There is no caller-supplied namespace argument.
/// CAS revision 0 creates; revisions increase and tombstones prevent ABA on delete/recreate.
pub trait PluginDocumentStore: Send + Sync {
    fn scope(&self) -> &PluginDataScope;
    fn get(&self, document_id: &str) -> Result<Option<PluginDocument>, PluginDataError>;
    fn compare_exchange(
        &self,
        expected_revision: u64,
        document: PluginDocument,
    ) -> Result<(), PluginDataError>;
    fn export_private(&self, maximum_bytes: usize) -> Result<Vec<PluginDocument>, PluginDataError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAsset {
    pub sha256: String,
    pub mime_type: String,
    pub byte_length: u64,
}

/// Content is bounded and scoped; IDs are not filesystem paths. GC follows document refs,
/// export pin and consumer leases. Uninstall alone never purges private data.
pub trait PluginAssetStore: Send + Sync {
    fn scope(&self) -> &PluginDataScope;
    fn put(&self, mime_type: &str, bytes: &[u8]) -> Result<PluginAsset, PluginDataError>;
    fn read(&self, asset: &PluginAsset, maximum_bytes: usize) -> Result<Vec<u8>, PluginDataError>;
}
