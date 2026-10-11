// SPDX-License-Identifier: GPL-3.0-or-later
//! Static C0 scanner for legacy OpenAction/Stream Deck bundles.
//! Runtime WebSocket translation remains an explicit later compatibility slice.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

const MAXIMUM_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LegacyManifest {
    pub name: String,
    pub author: String,
    pub version: String,
    pub category: String,
    pub icon: String,
    #[serde(default, rename = "OS")]
    pub os: Vec<LegacyOs>,
    #[serde(default)]
    pub code_paths: BTreeMap<String, String>,
    pub code_path_win: Option<String>,
    pub code_path_mac: Option<String>,
    pub code_path_lin: Option<String>,
    pub property_inspector_path: Option<String>,
    #[serde(default)]
    pub actions: Vec<LegacyAction>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LegacyOs {
    pub platform: String,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LegacyAction {
    #[serde(rename = "UUID")]
    pub uuid: String,
    pub name: String,
    pub icon: String,
    pub tooltip: Option<String>,
    #[serde(default)]
    pub controllers: Vec<String>,
    #[serde(default)]
    pub states: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityLevel {
    C0Indexed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum C1Readiness {
    ReadyForRuntimeHost,
    MissingCurrentPlatformArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedLegacyAction {
    pub uuid: String,
    pub name: String,
    pub controllers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityReport {
    pub level: CompatibilityLevel,
    pub name: String,
    pub author: String,
    pub version: String,
    pub actions: Vec<IndexedLegacyAction>,
    pub current_platform_entrypoint: Option<PathBuf>,
    pub c1_readiness: C1Readiness,
    pub has_property_inspector: bool,
    pub issues: Vec<CompatibilityIssue>,
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("legacy manifest I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("legacy manifest exceeds {MAXIMUM_MANIFEST_BYTES} bytes")]
    ManifestTooLarge,
    #[error("legacy manifest JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("legacy manifest field is empty: {0}")]
    EmptyField(&'static str),
    #[error("legacy action UUID is duplicated: {0}")]
    DuplicateAction(String),
    #[error("legacy path is absolute or escapes the bundle: {0}")]
    UnsafePath(String),
}

pub struct CompatibilityScanner;

impl CompatibilityScanner {
    /// Reads metadata only. It never starts CodePath, Node, a property
    /// inspector, an install hook, or arbitrary migration code.
    pub fn scan_bundle(bundle_root: &Path) -> Result<CompatibilityReport, ScanError> {
        let manifest_path = bundle_root.join("manifest.json");
        let metadata = fs::metadata(&manifest_path).map_err(|source| ScanError::Io {
            path: manifest_path.clone(),
            source,
        })?;
        if metadata.len() > MAXIMUM_MANIFEST_BYTES {
            return Err(ScanError::ManifestTooLarge);
        }
        let bytes = fs::read(&manifest_path).map_err(|source| ScanError::Io {
            path: manifest_path,
            source,
        })?;
        let manifest: LegacyManifest = serde_json::from_slice(&bytes)?;
        validate_manifest(&manifest)?;

        let relative_entrypoint = current_platform_entrypoint(&manifest);
        if let Some(path) = relative_entrypoint {
            validate_relative_path(path)?;
        }
        if let Some(path) = manifest.property_inspector_path.as_deref() {
            validate_relative_path(path)?;
        }
        let current_platform_entrypoint = relative_entrypoint.map(|path| bundle_root.join(path));
        let artifact_present = current_platform_entrypoint
            .as_ref()
            .is_some_and(|path| path.is_file());
        let mut issues = Vec::new();
        if !artifact_present {
            issues.push(CompatibilityIssue {
                code: "openaction.c1.artifact_missing".into(),
                message: "C0 metadata indexed; C1 needs a built artifact for this OS/architecture"
                    .into(),
            });
        }
        if manifest.property_inspector_path.is_some() {
            issues.push(CompatibilityIssue {
                code: "openaction.c2.property_inspector_deferred".into(),
                message: "Property Inspector requires a separate origin and scoped broker".into(),
            });
        }
        Ok(CompatibilityReport {
            level: CompatibilityLevel::C0Indexed,
            name: manifest.name,
            author: manifest.author,
            version: manifest.version,
            actions: manifest
                .actions
                .into_iter()
                .map(|action| IndexedLegacyAction {
                    uuid: action.uuid,
                    name: action.name,
                    controllers: action.controllers,
                })
                .collect(),
            current_platform_entrypoint,
            c1_readiness: if artifact_present {
                C1Readiness::ReadyForRuntimeHost
            } else {
                C1Readiness::MissingCurrentPlatformArtifact
            },
            has_property_inspector: manifest.property_inspector_path.is_some(),
            issues,
        })
    }
}

fn validate_manifest(manifest: &LegacyManifest) -> Result<(), ScanError> {
    for (name, value) in [
        ("Name", manifest.name.as_str()),
        ("Author", manifest.author.as_str()),
        ("Version", manifest.version.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ScanError::EmptyField(name));
        }
    }
    let mut action_ids = std::collections::BTreeSet::new();
    for action in &manifest.actions {
        if action.uuid.trim().is_empty() {
            return Err(ScanError::EmptyField("Actions[].UUID"));
        }
        if !action_ids.insert(&action.uuid) {
            return Err(ScanError::DuplicateAction(action.uuid.clone()));
        }
        validate_relative_path(&action.icon)?;
    }
    validate_relative_path(&manifest.icon)?;
    for path in manifest.code_paths.values() {
        validate_relative_path(path)?;
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), ScanError> {
    let path_value = Path::new(path);
    if path.is_empty()
        || path_value.is_absolute()
        || path_value.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ScanError::UnsafePath(path.into()));
    }
    Ok(())
}

fn current_platform_entrypoint(manifest: &LegacyManifest) -> Option<&str> {
    let target = format!("{}-{}", std::env::consts::ARCH, platform_target_suffix());
    manifest
        .code_paths
        .get(&target)
        .map(String::as_str)
        .or(match std::env::consts::OS {
            "windows" => manifest.code_path_win.as_deref(),
            "macos" => manifest.code_path_mac.as_deref(),
            "linux" => manifest.code_path_lin.as_deref(),
            _ => None,
        })
}

fn platform_target_suffix() -> &'static str {
    match std::env::consts::OS {
        "windows" => "pc-windows-msvc",
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        _ => "unknown",
    }
}
