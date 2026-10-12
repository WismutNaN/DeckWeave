// SPDX-License-Identifier: GPL-3.0-or-later
//! Проверенные шаблоны конфигурации, читаемые без запуска plugin.
use super::*;
use deckweave_application::plugin_catalog::PluginInstanceTemplate;
pub(super) fn read(
    package: &IndexedPackage,
    catalog: &deckweave_automation::DescriptorCatalog,
) -> Result<Vec<PluginInstanceTemplate>, PackageError> {
    let path = package.package_root.join(MANIFEST_NAME);
    let bytes = read_bounded(&path, MAX_MANIFEST_BYTES)?;
    if format!("{:x}", Sha256::digest(&bytes)) != package.manifest_sha256 {
        return Err(PackageError::HashMismatch { path });
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
    let templates: Vec<PluginInstanceTemplate> = manifest
        .get("org.deckweave.instanceTemplates")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|e| PackageError::InvalidManifest(e.to_string()))?
        .unwrap_or_default();
    let mut ids = std::collections::BTreeSet::new();
    if templates.len() > 64 {
        return Err(PackageError::InvalidManifest(
            "instance template budget".into(),
        ));
    }
    for template in &templates {
        if !ids.insert(template.contribution_id.clone())
            || !package
                .contributions
                .contains_key(&template.contribution_id)
            || catalog.schema(&template.settings_schema).is_none()
            || template.label_key.len() > 256
            || template.label_key.is_empty()
            || template.required_features.len() > 64
            || template.required_features.values().any(|v| *v == 0)
        {
            return Err(PackageError::InvalidManifest(
                "invalid instance template ownership/schema".into(),
            ));
        }
    }
    Ok(templates)
}
