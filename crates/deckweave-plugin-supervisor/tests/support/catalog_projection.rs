// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::plugin_catalog::*;

#[test]
fn projection_uses_locked_metadata_preserves_constraints_and_rejects_tampering()
-> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    create_package(temp.path())?;
    mutate_manifest(temp.path(), |m| {
        m["org.deckweave.automationCatalog"] = json!({
            "formatVersion":1, "endpoints":[], "schemas":[
                {"schema":{"id":"org.example.force","version":{"major":1,"minor":0}},
                 "valueKind":"number","unit":"N","range":{"minimum":0.0,"maximum":10.0,"step":null}},
                {"schema":{"id":"org.example.task","version":{"major":1,"minor":0}},
                 "valueKind":{"structured":{"type":"record","fields":{"title":{"schema":{"type":"string","maximumBytes":128,"choices":[]},"optional":false}}}},"unit":null,"range":null}
            ]
        });
        m["org.deckweave.translations"] =
            json!({"defaultLocale":"en", "messages":{"en":{"task":"Task"},"ru":{"task":"Задача"}}});
    })?;
    let package = PackageStager::inspect(temp.path(), &StagingLimits::default())?.package;
    let request = PluginCatalogRequest {
        package_id: package.package_id.clone(),
        version: package.version.to_string(),
        artifact_sha256: package
            .artifacts
            .values()
            .next()
            .ok_or("artifact")?
            .sha256
            .clone(),
    };
    let hash = package.manifest_sha256.clone();
    let mut index = PackageIndex::default();
    index.insert(package);
    let snapshot = index.catalog(&request)?;
    assert_eq!(snapshot.manifest_sha256, hash);
    assert_eq!(snapshot.schemas[0].unit.as_deref(), Some("N"));
    assert_eq!(
        snapshot.schemas[0].range.as_ref().ok_or("range")?.maximum,
        10.0
    );
    assert!(matches!(
        snapshot.schemas[1].shape,
        PluginValueShape::Structured(_)
    ));
    assert!(snapshot.translations.is_some());
    let json = serde_json::to_string(&snapshot)?;
    assert!(!json.contains("entrypoint") && !json.contains("packageRoot"));
    let mut missing = request.clone();
    missing.artifact_sha256 = "0".repeat(64);
    assert_eq!(index.catalog(&missing), Err(PluginCatalogError::NotFound));
    mutate_manifest(temp.path(), |m| m["version"] = json!("0.2.0"))?;
    assert_eq!(
        index.catalog(&request),
        Err(PluginCatalogError::InvalidCatalog)
    );
    Ok(())
}
