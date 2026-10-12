// SPDX-License-Identifier: GPL-3.0-or-later
//! Static, manifest-hashed automation metadata; inspection never starts code.
use super::*;
use deckweave_automation::{CatalogEndpoint, CatalogSchema, DescriptorCatalog};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StaticCatalog {
    format_version: u32,
    schemas: Vec<CatalogSchema>,
    endpoints: Vec<CatalogEndpoint>,
}

impl PackageIndex {
    pub fn descriptor_catalog(&self) -> Result<DescriptorCatalog, PackageError> {
        self.catalog_from(self.packages().collect())
    }

    pub fn locked_descriptor_catalog(
        &self,
        lock: &[deckweave_workspace::PackageLockEntry],
    ) -> Result<DescriptorCatalog, PackageError> {
        let mut selected = Vec::new();
        let mut packages = std::collections::BTreeSet::new();
        for entry in lock {
            if !packages.insert(entry.package_id.clone()) {
                return Err(PackageError::InvalidManifest(
                    "duplicate package lock".into(),
                ));
            }
            selected.push(
                self.resolve_artifact(&entry.package_id, &entry.version, &entry.artifact_sha256)
                    .ok_or_else(|| {
                        PackageError::InvalidManifest("missing or ambiguous locked package".into())
                    })?,
            );
        }
        self.catalog_from(selected)
    }

    fn catalog_from(
        &self,
        selected: Vec<&IndexedPackage>,
    ) -> Result<DescriptorCatalog, PackageError> {
        let mut schemas: Vec<CatalogSchema> = Vec::new();
        let mut endpoints = Vec::new();
        for package in selected {
            let path = package.package_root.join(MANIFEST_NAME);
            let bytes = read_bounded(&path, MAX_MANIFEST_BYTES)?;
            if format!("{:x}", Sha256::digest(&bytes)) != package.manifest_sha256 {
                return Err(PackageError::HashMismatch { path });
            }
            let manifest: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
            let Some(value) = manifest.get("org.deckweave.automationCatalog") else {
                continue;
            };
            let catalog: StaticCatalog = serde_json::from_value(value.clone())
                .map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
            if catalog.format_version != 1
                || catalog.endpoints.len() > 4096
                || catalog.schemas.len() > 4096
            {
                return Err(PackageError::InvalidManifest(
                    "catalog version or budget".into(),
                ));
            }
            for entry in &catalog.endpoints {
                let requirement = &entry.runtime_requirement;
                let contribution = package
                    .contributions
                    .get(&requirement.contribution_id)
                    .ok_or_else(|| {
                        PackageError::InvalidManifest("catalog contribution absent".into())
                    })?;
                let artifact =
                    package.artifact_for_current_platform(&requirement.runtime_group_id)?;
                if entry.endpoint.package_id != package.package_id
                    || requirement.package_id != package.package_id
                    || requirement.package_version != package.version.to_string()
                    || contribution.runtime_group_id != requirement.runtime_group_id
                    || artifact.sha256 != requirement.artifact_sha256
                    || requirement.configuration.is_some()
                {
                    return Err(PackageError::InvalidManifest(
                        "catalog ownership/artifact mismatch".into(),
                    ));
                }
            }
            for schema in catalog.schemas {
                if let Some(existing) = schemas.iter().find(|s| s.schema == schema.schema) {
                    if existing != &schema {
                        return Err(PackageError::InvalidManifest(
                            "conflicting shared schema".into(),
                        ));
                    }
                } else {
                    schemas.push(schema);
                }
            }
            endpoints.extend(catalog.endpoints);
        }
        DescriptorCatalog::try_new(schemas, endpoints)
            .map_err(|e| PackageError::InvalidManifest(e.to_string()))
    }
}
