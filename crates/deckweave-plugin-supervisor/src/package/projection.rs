// SPDX-License-Identifier: GPL-3.0-or-later
//! Application projection uses the same validated static catalogue as the compiler.
use super::*;
use deckweave_application::plugin_catalog::*;
use deckweave_automation::SchemaValueKind;

impl PluginCatalogSource for PackageIndex {
    fn catalog(
        &self,
        request: &PluginCatalogRequest,
    ) -> Result<PluginCatalogSnapshot, PluginCatalogError> {
        if !request.is_bounded() {
            return Err(PluginCatalogError::InvalidCatalog);
        }
        let package = self
            .resolve_artifact(
                &request.package_id,
                &request.version,
                &request.artifact_sha256,
            )
            .ok_or(PluginCatalogError::NotFound)?;
        let lock = deckweave_workspace::PackageLockEntry {
            package_id: request.package_id.clone(),
            version: request.version.clone(),
            artifact_sha256: request.artifact_sha256.clone(),
            source: None,
            extensions: BTreeMap::new(),
        };
        let catalog = self.locked_descriptor_catalog(&[lock]).map_err(map_error)?;
        let translations = self
            .translation_catalog(
                &request.package_id,
                &request.version,
                &request.artifact_sha256,
            )
            .map_err(map_error)?;
        Ok(PluginCatalogSnapshot {
            format_version: 1,
            package: request.clone(),
            manifest_sha256: package.manifest_sha256.clone(),
            schemas: catalog
                .schemas()
                .iter()
                .map(|s| PluginSchemaView {
                    schema: s.schema.clone(),
                    unit: s.unit.clone(),
                    range: s.range.clone(),
                    shape: match &s.value_kind {
                        SchemaValueKind::Unit => PluginValueShape::Unit,
                        SchemaValueKind::Boolean => PluginValueShape::Boolean,
                        SchemaValueKind::Number => PluginValueShape::Number,
                        SchemaValueKind::String => PluginValueShape::String,
                        SchemaValueKind::Json => PluginValueShape::Json,
                        SchemaValueKind::Structured(shape) => {
                            PluginValueShape::Structured(shape.clone())
                        }
                    },
                })
                .collect(),
            endpoints: catalog
                .endpoints()
                .iter()
                .map(|e| PluginEndpointView {
                    endpoint: e.endpoint.clone(),
                    contribution_id: e.runtime_requirement.contribution_id.clone(),
                    runtime_group_id: e.runtime_requirement.runtime_group_id.clone(),
                    port: e.port.clone(),
                    operation: e.operation.clone(),
                })
                .collect(),
            translations,
        })
    }
}
fn map_error(error: PackageError) -> PluginCatalogError {
    match error {
        PackageError::Io { .. } => PluginCatalogError::Unavailable,
        _ => PluginCatalogError::InvalidCatalog,
    }
}
