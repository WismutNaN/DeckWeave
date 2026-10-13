// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};

const ID: &str = "org.deckweave.fixture";
fn package(root: &Path, version: &str, executable: &[u8]) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(root.join("bin"))?;
    fs::write(root.join("bin/plugin"), executable)?;
    fs::write(
        root.join("deckweave-plugin.json"),
        serde_json::to_vec(&json!({
            "manifestVersion":1,"packageId":ID,"version":version,"license":"GPL-3.0-or-later",
            "source":{"repository":"https://example.invalid/fixture","revision":"test-fixture"},
            "artifacts":[{"id":"native","os":std::env::consts::OS,"architecture":std::env::consts::ARCH,
                "entrypoint":"bin/plugin","sha256":format!("{:x}", Sha256::digest(executable))}],
            "runtimeGroups":[{"id":"control","runtimeClass":"isolated_control","artifactId":"native"}],
            "contributions":[{"id":"fixture","type":"org.deckweave.fixture","runtimeGroupId":"control","descriptor":{}}],
            "protocolRange":{"minimum":{"major":1,"minor":0},"maximum":{"major":1,"minor":1}},
            "permissions":[],"schemas":[]
        }))?,
    )?;
    PackageStager::inspect(root, &StagingLimits::default())?;
    Ok(())
}

#[test]
fn versions_install_side_by_side_and_pinned_content_is_never_replaced() -> Result<(), Box<dyn Error>>
{
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source");
    let root = dir.path().join("data");
    package(&source, "0.1.0", b"original")?;
    install(&root, &source, "fixture", ID)?;
    let old = root.join("packages/fixture/0.1.0/content");
    let manifest = fs::read(old.join("deckweave-plugin.json"))?;
    package(&source, "0.1.0", b"same-version rebuild")?;
    install(&root, &source, "fixture", ID)?;
    assert_eq!(fs::read(old.join("bin/plugin"))?, b"original");
    assert_eq!(fs::read(old.join("deckweave-plugin.json"))?, manifest);
    package(&source, "0.2.0", b"next")?;
    install(&root, &source, "fixture", ID)?;
    assert_eq!(
        fs::read(root.join("packages/fixture/0.2.0/content/bin/plugin"))?,
        b"next"
    );
    assert_eq!(fs::read(old.join("bin/plugin"))?, b"original");
    let index = deckweave_plugin_supervisor::package::PackageIndex::scan(
        &root.join("packages"),
        &StagingLimits::default(),
    )?;
    assert_eq!(index.packages().count(), 2);
    assert!(fs::read_dir(&root)?.all(|entry| entry.is_ok_and(|e| e.file_name() == "packages")));
    Ok(())
}

#[test]
fn failed_publication_cleans_only_its_temporary_directory() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source");
    let root = dir.path().join("data");
    package(&source, "0.1.0", b"first")?;
    fs::create_dir_all(&root)?;
    // A destination conflict happens after the staged copy has been verified.
    fs::write(root.join("packages"), b"must survive")?;
    assert!(install(&root, &source, "fixture", ID).is_err());
    assert_eq!(fs::read(root.join("packages"))?, b"must survive");
    assert_eq!(fs::read_dir(&root)?.count(), 1);
    assert_eq!(fs::read(source.join("bin/plugin"))?, b"first");
    Ok(())
}

#[test]
fn corrupt_bundle_cannot_publish_or_replace_existing_content() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source");
    let root = dir.path().join("data");
    package(&source, "0.1.0", b"first")?;
    install(&root, &source, "fixture", ID)?;
    package(&source, "0.2.0", b"second")?;
    fs::write(source.join("bin/plugin"), b"tampered")?;
    assert!(install(&root, &source, "fixture", ID).is_err());
    assert!(!root.join("packages/fixture/0.2.0").exists());
    assert_eq!(
        fs::read(root.join("packages/fixture/0.1.0/content/bin/plugin"))?,
        b"first"
    );
    Ok(())
}
