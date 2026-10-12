// SPDX-License-Identifier: GPL-3.0-or-later
//! Read projections for clients; validation and compilation remain with their owners.
use deckweave_domain::{
    automation::OperationDescriptor,
    capability::{NumericRange, PortDescriptor},
    endpoint::StableEndpointRef,
    ids::{ContributionId, PackageId, RuntimeGroupId},
    localization::TranslationCatalog,
    schema::{PayloadSchema, SchemaRef},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCatalogRequest {
    pub package_id: PackageId,
    pub version: String,
    pub artifact_sha256: String,
}
impl PluginCatalogRequest {
    pub fn is_bounded(&self) -> bool {
        !self.version.is_empty()
            && self.version.len() <= 128
            && self.artifact_sha256.len() == 64
            && self.artifact_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginValueShape {
    Unit,
    Boolean,
    Number,
    String,
    Json,
    Structured(PayloadSchema),
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSchemaView {
    pub schema: SchemaRef,
    pub shape: PluginValueShape,
    pub unit: Option<String>,
    pub range: Option<NumericRange>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEndpointView {
    pub endpoint: StableEndpointRef,
    pub contribution_id: ContributionId,
    pub runtime_group_id: RuntimeGroupId,
    pub port: PortDescriptor,
    pub operation: Option<OperationDescriptor>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCatalogSnapshot {
    pub format_version: u32,
    pub package: PluginCatalogRequest,
    pub manifest_sha256: String,
    pub schemas: Vec<PluginSchemaView>,
    pub endpoints: Vec<PluginEndpointView>,
    pub translations: Option<TranslationCatalog>,
}
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginCatalogError {
    #[error("locked package missing or ambiguous")]
    NotFound,
    #[error("static package metadata failed validation")]
    InvalidCatalog,
    #[error("plugin catalogue is unavailable")]
    Unavailable,
}
pub trait PluginCatalogSource: Send + Sync {
    fn catalog(
        &self,
        request: &PluginCatalogRequest,
    ) -> Result<PluginCatalogSnapshot, PluginCatalogError>;
}
pub struct UnavailablePluginCatalog;
impl PluginCatalogSource for UnavailablePluginCatalog {
    fn catalog(
        &self,
        _request: &PluginCatalogRequest,
    ) -> Result<PluginCatalogSnapshot, PluginCatalogError> {
        Err(PluginCatalogError::Unavailable)
    }
}
