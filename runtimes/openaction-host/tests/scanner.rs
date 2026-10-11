// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_openaction_host::{C1Readiness, CompatibilityLevel, CompatibilityScanner, ScanError};
use std::{error::Error, fs, path::PathBuf};

fn pinned_counter_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/compatibility/openaction-counter")
}

#[test]
fn pinned_counter_is_c0_indexed_without_starting_code() -> Result<(), Box<dyn Error>> {
    let report = CompatibilityScanner::scan_bundle(&pinned_counter_fixture())?;
    assert_eq!(report.level, CompatibilityLevel::C0Indexed);
    assert_eq!(report.name, "Counter");
    assert_eq!(report.version, "1.0.1");
    assert_eq!(report.actions.len(), 2);
    assert!(
        report
            .actions
            .iter()
            .all(|action| { action.controllers == ["Keypad".to_string(), "Encoder".to_string()] })
    );
    assert_eq!(
        report.c1_readiness,
        C1Readiness::MissingCurrentPlatformArtifact
    );
    assert!(report.has_property_inspector);
    Ok(())
}

#[test]
fn unsafe_legacy_paths_and_duplicate_actions_are_rejected() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = fs::read_to_string(pinned_counter_fixture().join("manifest.json"))?;
    let escaped = fixture.replace("\"Icon\": \"icon\"", "\"Icon\": \"../icon\"");
    fs::write(directory.path().join("manifest.json"), escaped)?;
    assert!(matches!(
        CompatibilityScanner::scan_bundle(directory.path()),
        Err(ScanError::UnsafePath(_))
    ));

    let mut duplicated: serde_json::Value = serde_json::from_str(&fixture)?;
    duplicated["Actions"][1]["UUID"] = duplicated["Actions"][0]["UUID"].clone();
    fs::write(
        directory.path().join("manifest.json"),
        serde_json::to_vec(&duplicated)?,
    )?;
    assert!(matches!(
        CompatibilityScanner::scan_bundle(directory.path()),
        Err(ScanError::DuplicateAction(_))
    ));
    Ok(())
}
