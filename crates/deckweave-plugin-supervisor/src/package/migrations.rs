// SPDX-License-Identifier: GPL-3.0-or-later
//! A hash-verified declaration only; the editor checks every old/new endpoint contract.
use super::*;
use deckweave_application::plugin_catalog::EndpointMigrationSource;
pub(super) fn read(package: &IndexedPackage) -> Result<Vec<EndpointMigrationSource>, PackageError> {
    let path = package.package_root.join(MANIFEST_NAME);
    let bytes = read_bounded(&path, MAX_MANIFEST_BYTES)?;
    if format!("{:x}", Sha256::digest(&bytes)) != package.manifest_sha256 {
        return Err(PackageError::HashMismatch { path });
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| PackageError::InvalidManifest(e.to_string()))?;
    let sources: Vec<EndpointMigrationSource> = manifest
        .get("org.deckweave.endpointMigrations")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|e| PackageError::InvalidManifest(e.to_string()))?
        .unwrap_or_default();
    let mut distinct = std::collections::BTreeSet::new();
    if sources.len() > 16
        || sources.iter().any(|s| {
            s.package_id == package.package_id
                || semver::Version::parse(&s.version).is_err()
                || s.version.len() > 128
                || !distinct.insert((&s.package_id, &s.version))
        })
    {
        return Err(PackageError::InvalidManifest(
            "endpoint migration source/version".into(),
        ));
    }
    Ok(sources)
}
