// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_domain::localization::TranslationCatalog;

impl PackageIndex {
    /// Read static, bounded metadata from the selected package; never execute plugin code.
    pub fn translation_catalog(
        &self,
        package_id: &PackageId,
        version: &str,
        artifact_sha256: &str,
    ) -> Result<Option<TranslationCatalog>, PackageError> {
        let package = self
            .resolve_artifact(package_id, version, artifact_sha256)
            .ok_or_else(|| {
                PackageError::InvalidManifest("missing or ambiguous locked package".into())
            })?;
        let path = package.package_root.join(MANIFEST_NAME);
        let bytes = read_bounded(&path, MAX_MANIFEST_BYTES)?;
        if format!("{:x}", Sha256::digest(&bytes)) != package.manifest_sha256 {
            return Err(PackageError::HashMismatch { path });
        }
        let manifest: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
        let Some(value) = manifest.get("org.deckweave.translations") else {
            return Ok(None);
        };
        let catalog: TranslationCatalog = serde_json::from_value(value.clone())
            .map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
        catalog
            .validate()
            .map_err(|e| PackageError::InvalidManifest(e.into()))?;
        Ok(Some(catalog))
    }
}
