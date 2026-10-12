// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_plugin_supervisor::package::{
    PackageError, PackageIndex, PackageStager, StagingLimits,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::Path};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn platform() -> (&'static str, &'static str) {
    (std::env::consts::OS, std::env::consts::ARCH)
}

fn create_package(root: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(root.join("bin"))?;
    fs::create_dir_all(root.join("schema"))?;
    let executable = b"not executable and indexing must never try to run it";
    let schema = br#"{"type":"object","additionalProperties":{}}"#;
    fs::write(root.join("bin/plugin.bin"), executable)?;
    fs::write(root.join("schema/settings.json"), schema)?;
    let (os, architecture) = platform();
    let manifest = json!({
        "manifestVersion": 1,
        "packageId": "org.deckweave.fixture.virtual",
        "version": "0.1.0",
        "license": "MIT OR Apache-2.0",
        "source": {
            "repository": "https://example.invalid/deckweave/virtual",
            "revision": "0123456789abcdef"
        },
        "artifacts": [{
            "id": "current-platform",
            "os": os,
            "architecture": architecture,
            "entrypoint": "bin/plugin.bin",
            "sha256": digest(executable)
        }],
        "runtimeGroups": [{
            "id": "control",
            "runtimeClass": "isolated_control",
            "artifactId": "current-platform"
        }],
        "contributions": [{
            "id": "virtual-device",
            "type": "org.deckweave.device_provider",
            "runtimeGroupId": "control",
            "descriptor": { "kind": "virtual" }
        }],
        "protocolRange": {
            "minimum": { "major": 1, "minor": 0 },
            "maximum": { "major": 1, "minor": 1 }
        },
        "permissions": [{
            "scope": "org.deckweave.device.virtual",
            "accessMode": "execute",
            "resourceSelector": { "deviceKind": "virtual" },
            "reason": "Publish a deterministic virtual device",
            "optional": false,
            "runtimeGroupIds": ["control"]
        }],
        "schemas": [{
            "id": "org.deckweave.fixture.settings",
            "version": { "major": 1, "minor": 0 },
            "path": "schema/settings.json",
            "sha256": digest(schema)
        }]
    });
    fs::write(
        root.join("deckweave-plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn mutate_manifest(root: &Path, mutation: impl FnOnce(&mut Value)) -> Result<(), Box<dyn Error>> {
    let path = root.join("deckweave-plugin.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
    mutation(&mut manifest);
    fs::write(path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

#[test]
fn indexing_validates_without_executing_entrypoint() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let package_root = temp.path().join("incoming/virtual");
    create_package(&package_root)?;
    let staged = PackageStager::inspect(&package_root, &StagingLimits::default())?;
    assert_eq!(staged.file_count, 3);
    assert_eq!(
        staged.package.package_id.as_str(),
        "org.deckweave.fixture.virtual"
    );
    assert_eq!(staged.package.contributions.len(), 1);
    let runtime_group = staged
        .package
        .runtime_groups
        .keys()
        .next()
        .ok_or("runtime group missing")?;
    let artifact = staged
        .package
        .artifact_for_current_platform(runtime_group)?;
    assert_eq!(
        fs::read(&artifact.entrypoint)?,
        b"not executable and indexing must never try to run it"
    );
    Ok(())
}

#[test]
fn static_catalog_checks_manifest_hash_and_rejects_unknown_required_fields()
-> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    create_package(temp.path())?;
    mutate_manifest(temp.path(), |m| {
        m["org.deckweave.automationCatalog"] =
            json!({"formatVersion":1,"schemas":[],"endpoints":[]})
    })?;
    let staged = PackageStager::inspect(temp.path(), &StagingLimits::default())?;
    let mut index = PackageIndex::default();
    index.insert(staged.package);
    assert!(index.descriptor_catalog()?.endpoints().is_empty());
    mutate_manifest(temp.path(), |m| {
        m["org.deckweave.automationCatalog"]["unsupportedRequired"] = json!(true)
    })?;
    assert!(matches!(
        index.descriptor_catalog(),
        Err(PackageError::HashMismatch { .. })
    ));
    let mut index = PackageIndex::default();
    index.insert(PackageStager::inspect(temp.path(), &StagingLimits::default())?.package);
    assert!(matches!(
        index.descriptor_catalog(),
        Err(PackageError::InvalidManifest(_))
    ));
    Ok(())
}

#[test]
fn hash_tampering_and_invalid_references_are_rejected() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let package_root = temp.path().join("virtual");
    create_package(&package_root)?;
    fs::write(package_root.join("bin/plugin.bin"), b"tampered")?;
    assert!(matches!(
        PackageStager::inspect(&package_root, &StagingLimits::default()),
        Err(PackageError::HashMismatch { .. })
    ));

    create_package(&package_root)?;
    let staged = PackageStager::inspect(&package_root, &StagingLimits::default())?;
    fs::write(
        package_root.join("bin/plugin.bin"),
        b"tampered after indexing",
    )?;
    assert!(matches!(
        staged.package.verify_artifact_for_current_platform(
            &deckweave_domain::ids::RuntimeGroupId::try_new("control")?,
            StagingLimits::default().maximum_file_bytes,
        ),
        Err(PackageError::HashMismatch { .. })
    ));

    create_package(&package_root)?;
    mutate_manifest(&package_root, |manifest| {
        manifest["runtimeGroups"][0]["artifactId"] = json!("missing-artifact");
    })?;
    assert!(matches!(
        PackageStager::inspect(&package_root, &StagingLimits::default()),
        Err(PackageError::UnknownArtifact(_))
    ));
    Ok(())
}

#[test]
fn unsafe_paths_and_staging_limits_are_rejected() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let package_root = temp.path().join("virtual");
    create_package(&package_root)?;
    mutate_manifest(&package_root, |manifest| {
        manifest["artifacts"][0]["entrypoint"] = json!("../outside.exe");
    })?;
    assert!(matches!(
        PackageStager::inspect(&package_root, &StagingLimits::default()),
        Err(PackageError::InvalidManifest(_)) | Err(PackageError::UnsafePath(_))
    ));

    create_package(&package_root)?;
    let limits = StagingLimits {
        maximum_files: 2,
        ..StagingLimits::default()
    };
    assert!(matches!(
        PackageStager::inspect(&package_root, &limits),
        Err(PackageError::StagingLimit("maximum files"))
    ));
    Ok(())
}

#[test]
fn recursive_index_keeps_exact_version_and_hash() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let package_root = temp
        .path()
        .join("org.deckweave.fixture.virtual/0.1.0/content");
    create_package(&package_root)?;
    let index = PackageIndex::scan(temp.path(), &StagingLimits::default())?;
    let packages: Vec<_> = index.packages().collect();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].version.to_string(), "0.1.0");
    assert_eq!(packages[0].manifest_sha256.len(), 64);
    Ok(())
}

#[test]
fn static_translations_use_parent_default_and_explicit_fallback_without_running_code()
-> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    create_package(temp.path())?;
    mutate_manifest(temp.path(), |manifest| {
        manifest["org.deckweave.translations"] = json!({"defaultLocale":"en", "messages":{
            "en":{"title":"Clock", "onlyEnglish":"Time"}, "ru":{"title":"Часы"}
        }});
    })?;
    let index = PackageIndex::scan(temp.path(), &StagingLimits::default())?;
    let package = index.packages().next().ok_or("package")?;
    let artifact = package.artifacts.values().next().ok_or("artifact")?;
    let catalog = index
        .translation_catalog(&package.package_id, "0.1.0", &artifact.sha256)?
        .ok_or("catalog")?;
    assert_eq!(catalog.resolve("ru-RU", "title", "fallback"), "Часы");
    assert_eq!(catalog.resolve("ru", "onlyEnglish", "fallback"), "Time");
    assert_eq!(catalog.resolve("zz", "title", "fallback"), "Clock");
    assert_eq!(catalog.resolve("ru", "missing", "fallback"), "fallback");
    mutate_manifest(temp.path(), |manifest| {
        manifest["org.deckweave.translations"]["messages"]["ru"]["title"] = json!("changed");
    })?;
    assert!(matches!(
        index.translation_catalog(&package.package_id, "0.1.0", &artifact.sha256),
        Err(PackageError::HashMismatch { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_is_rejected_even_when_target_stays_inside_package() -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir()?;
    let package_root = temp.path().join("virtual");
    create_package(&package_root)?;
    fs::rename(
        package_root.join("schema/settings.json"),
        package_root.join("schema/real-settings.json"),
    )?;
    symlink(
        "real-settings.json",
        package_root.join("schema/settings.json"),
    )?;
    assert!(matches!(
        PackageStager::inspect(&package_root, &StagingLimits::default()),
        Err(PackageError::UnsafePath(_))
    ));
    Ok(())
}
