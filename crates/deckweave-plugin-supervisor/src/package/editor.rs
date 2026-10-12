// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::{editor::InstalledPluginView, plugin_catalog::PluginCatalogRequest};
use deckweave_domain::capability::NodeDescriptor;

impl PackageIndex {
    pub fn editor_packages(&self) -> Result<Vec<InstalledPluginView>, PackageError> {
        let mut result = Vec::new();
        for package in self.packages() {
            let mut hashes = std::collections::BTreeSet::new();
            for artifact in package.artifacts.values().filter(|a| {
                a.operating_system == std::env::consts::OS
                    && a.architecture == std::env::consts::ARCH
            }) {
                if !hashes.insert(artifact.sha256.clone()) {
                    continue;
                }
                if result.len() >= 64 {
                    return Err(PackageError::InvalidManifest(
                        "editor package limit exceeded".into(),
                    ));
                }
                let path = package.package_root.join(MANIFEST_NAME);
                let bytes = read_bounded(&path, MAX_MANIFEST_BYTES)?;
                if format!("{:x}", Sha256::digest(&bytes)) != package.manifest_sha256 {
                    return Err(PackageError::HashMismatch { path });
                }
                let manifest: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
                let templates: Vec<NodeDescriptor> = manifest
                    .get("org.deckweave.deviceTemplates")
                    .map(|v| serde_json::from_value(v.clone()))
                    .transpose()
                    .map_err(|e| PackageError::InvalidManifest(e.to_string()))?
                    .unwrap_or_default();
                if templates.len() > 16 {
                    return Err(PackageError::InvalidManifest(
                        "device template limit exceeded".into(),
                    ));
                }
                for template in &templates {
                    template
                        .validate()
                        .map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
                }
                result.push(InstalledPluginView {
                    package: PluginCatalogRequest {
                        package_id: package.package_id.clone(),
                        version: package.version.to_string(),
                        artifact_sha256: artifact.sha256.clone(),
                    },
                    manifest_sha256: package.manifest_sha256.clone(),
                    templates,
                });
            }
        }
        Ok(result)
    }
}
