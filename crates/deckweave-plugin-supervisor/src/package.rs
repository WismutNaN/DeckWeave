// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{
    ids::{ArtifactId, ContributionId, NamespacedKind, PackageId, RuntimeGroupId},
    security::PermissionAccessMode,
};
use deckweave_protocol::documents::plugin_manifest::PluginManifest;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

const MANIFEST_NAME: &str = "deckweave-plugin.json";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
mod lookup;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagingLimits {
    pub maximum_files: usize,
    pub maximum_total_bytes: u64,
    pub maximum_file_bytes: u64,
}

impl Default for StagingLimits {
    fn default() -> Self {
        Self {
            maximum_files: 4096,
            maximum_total_bytes: 512 * 1024 * 1024,
            maximum_file_bytes: 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedArtifact {
    pub artifact_id: ArtifactId,
    pub operating_system: String,
    pub architecture: String,
    pub entrypoint: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedRuntimeGroup {
    pub runtime_group_id: RuntimeGroupId,
    pub runtime_class: String,
    pub artifact_id: ArtifactId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedContribution {
    pub contribution_id: ContributionId,
    pub contribution_type: NamespacedKind,
    pub runtime_group_id: RuntimeGroupId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedPermission {
    pub scope: NamespacedKind,
    pub access_mode: PermissionAccessMode,
    pub resource_selector: serde_json::Value,
    pub reason: String,
    pub optional: bool,
    pub runtime_group_ids: Vec<RuntimeGroupId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedPackage {
    pub package_id: PackageId,
    pub version: Version,
    pub package_root: PathBuf,
    pub manifest_sha256: String,
    pub artifacts: BTreeMap<ArtifactId, IndexedArtifact>,
    pub runtime_groups: BTreeMap<RuntimeGroupId, IndexedRuntimeGroup>,
    pub contributions: BTreeMap<ContributionId, IndexedContribution>,
    pub requested_permissions: Vec<RequestedPermission>,
}

impl IndexedPackage {
    pub fn artifact_for_current_platform(
        &self,
        runtime_group: &RuntimeGroupId,
    ) -> Result<&IndexedArtifact, PackageError> {
        let group = self
            .runtime_groups
            .get(runtime_group)
            .ok_or_else(|| PackageError::UnknownRuntimeGroup(runtime_group.to_string()))?;
        let artifact = self
            .artifacts
            .get(&group.artifact_id)
            .ok_or_else(|| PackageError::UnknownArtifact(group.artifact_id.to_string()))?;
        let operating_system = std::env::consts::OS;
        let architecture = std::env::consts::ARCH;
        if artifact.operating_system != operating_system || artifact.architecture != architecture {
            return Err(PackageError::UnsupportedPlatform {
                operating_system: operating_system.into(),
                architecture: architecture.into(),
            });
        }
        Ok(artifact)
    }

    /// Revalidates the immutable artifact immediately before execution. The
    /// index is metadata, not authority to execute bytes changed afterwards.
    pub fn verify_artifact_for_current_platform(
        &self,
        runtime_group: &RuntimeGroupId,
        maximum_artifact_bytes: u64,
    ) -> Result<&IndexedArtifact, PackageError> {
        let artifact = self.artifact_for_current_platform(runtime_group)?;
        verify_hash(
            &artifact.entrypoint,
            &artifact.sha256,
            maximum_artifact_bytes,
        )?;
        Ok(artifact)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedPackage {
    pub package: IndexedPackage,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Default)]
pub struct PackageIndex {
    packages: BTreeMap<(PackageId, Version, String), IndexedPackage>,
}

impl PackageIndex {
    pub fn insert(&mut self, package: IndexedPackage) -> Option<IndexedPackage> {
        let key = (
            package.package_id.clone(),
            package.version.clone(),
            package.manifest_sha256.clone(),
        );
        self.packages.insert(key, package)
    }

    pub fn packages(&self) -> impl Iterator<Item = &IndexedPackage> {
        self.packages.values()
    }

    pub fn scan(installed_root: &Path, limits: &StagingLimits) -> Result<Self, PackageError> {
        let mut manifests = Vec::new();
        collect_manifests(installed_root, 0, &mut manifests)?;
        let mut index = Self::default();
        for manifest in manifests {
            let package_root = manifest
                .parent()
                .ok_or_else(|| PackageError::UnsafePath(manifest.clone()))?;
            index.insert(PackageStager::inspect(package_root, limits)?.package);
        }
        Ok(index)
    }
}

pub struct PackageStager;

impl PackageStager {
    /// Inspects an already-created staging directory. It never executes an
    /// entrypoint, install hook, migration, or compatibility command.
    pub fn inspect(root: &Path, limits: &StagingLimits) -> Result<StagedPackage, PackageError> {
        let root_metadata = fs::symlink_metadata(root).map_err(|source| PackageError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(PackageError::UnsafePath(root.to_path_buf()));
        }
        let root = fs::canonicalize(root).map_err(|source| PackageError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let mut budget = StagingBudget::default();
        inspect_tree(&root, &root, limits, &mut budget)?;
        let manifest_path = root.join(MANIFEST_NAME);
        let manifest_bytes = read_bounded(&manifest_path, MAX_MANIFEST_BYTES)?;
        let _: PluginManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| PackageError::InvalidManifest(error.to_string()))?;
        let manifest: ManifestView = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| PackageError::InvalidManifest(error.to_string()))?;
        let package = index_manifest(&root, &manifest_bytes, manifest, limits)?;
        Ok(StagedPackage {
            package,
            file_count: budget.files,
            total_bytes: budget.bytes,
        })
    }
}

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("I/O error for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("manifest schema/JSON invalid: {0}")]
    InvalidManifest(String),
    #[error("package path is unsafe or escapes staging root: {0}")]
    UnsafePath(PathBuf),
    #[error("staging limit exceeded: {0}")]
    StagingLimit(&'static str),
    #[error("SHA-256 mismatch for {path}")]
    HashMismatch { path: PathBuf },
    #[error("duplicate manifest ID: {0}")]
    DuplicateId(String),
    #[error("runtime group references unknown artifact: {0}")]
    UnknownArtifact(String),
    #[error("contribution references unknown runtime group: {0}")]
    UnknownRuntimeGroup(String),
    #[error("artifact is not available for {operating_system}/{architecture}")]
    UnsupportedPlatform {
        operating_system: String,
        architecture: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestView {
    package_id: PackageId,
    version: Version,
    artifacts: Vec<ArtifactView>,
    runtime_groups: Vec<RuntimeGroupView>,
    contributions: Vec<ContributionView>,
    permissions: Vec<PermissionView>,
    schemas: Vec<SchemaView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactView {
    id: ArtifactId,
    os: String,
    architecture: String,
    entrypoint: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeGroupView {
    id: RuntimeGroupId,
    runtime_class: String,
    artifact_id: ArtifactId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContributionView {
    id: ContributionId,
    #[serde(rename = "type")]
    contribution_type: NamespacedKind,
    runtime_group_id: RuntimeGroupId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionView {
    scope: NamespacedKind,
    access_mode: PermissionAccessMode,
    resource_selector: serde_json::Value,
    reason: String,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    runtime_group_ids: Vec<RuntimeGroupId>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchemaView {
    path: String,
    sha256: String,
}

fn index_manifest(
    root: &Path,
    manifest_bytes: &[u8],
    manifest: ManifestView,
    limits: &StagingLimits,
) -> Result<IndexedPackage, PackageError> {
    let mut artifacts = BTreeMap::new();
    for artifact in manifest.artifacts {
        let path = resolve_package_file(root, &artifact.entrypoint)?;
        verify_hash(&path, &artifact.sha256, limits.maximum_file_bytes)?;
        let indexed = IndexedArtifact {
            artifact_id: artifact.id.clone(),
            operating_system: artifact.os,
            architecture: artifact.architecture,
            entrypoint: path,
            sha256: artifact.sha256.to_ascii_lowercase(),
        };
        if artifacts.insert(artifact.id.clone(), indexed).is_some() {
            return Err(PackageError::DuplicateId(artifact.id.to_string()));
        }
    }

    for schema in manifest.schemas {
        let path = resolve_package_file(root, &schema.path)?;
        verify_hash(&path, &schema.sha256, limits.maximum_file_bytes)?;
    }

    let mut runtime_groups = BTreeMap::new();
    for group in manifest.runtime_groups {
        if !artifacts.contains_key(&group.artifact_id) {
            return Err(PackageError::UnknownArtifact(group.artifact_id.to_string()));
        }
        let indexed = IndexedRuntimeGroup {
            runtime_group_id: group.id.clone(),
            runtime_class: group.runtime_class,
            artifact_id: group.artifact_id,
        };
        if runtime_groups.insert(group.id.clone(), indexed).is_some() {
            return Err(PackageError::DuplicateId(group.id.to_string()));
        }
    }

    let mut contributions = BTreeMap::new();
    for contribution in manifest.contributions {
        if !runtime_groups.contains_key(&contribution.runtime_group_id) {
            return Err(PackageError::UnknownRuntimeGroup(
                contribution.runtime_group_id.to_string(),
            ));
        }
        let indexed = IndexedContribution {
            contribution_id: contribution.id.clone(),
            contribution_type: contribution.contribution_type,
            runtime_group_id: contribution.runtime_group_id,
        };
        if contributions
            .insert(contribution.id.clone(), indexed)
            .is_some()
        {
            return Err(PackageError::DuplicateId(contribution.id.to_string()));
        }
    }

    let mut requested_permissions = Vec::new();
    for permission in manifest.permissions {
        for runtime_group_id in &permission.runtime_group_ids {
            if !runtime_groups.contains_key(runtime_group_id) {
                return Err(PackageError::UnknownRuntimeGroup(
                    runtime_group_id.to_string(),
                ));
            }
        }
        requested_permissions.push(RequestedPermission {
            scope: permission.scope,
            access_mode: permission.access_mode,
            resource_selector: permission.resource_selector,
            reason: permission.reason,
            optional: permission.optional,
            runtime_group_ids: permission.runtime_group_ids,
        });
    }

    Ok(IndexedPackage {
        package_id: manifest.package_id,
        version: manifest.version,
        package_root: root.to_path_buf(),
        manifest_sha256: digest_bytes(manifest_bytes),
        artifacts,
        runtime_groups,
        contributions,
        requested_permissions,
    })
}

fn collect_manifests(
    root: &Path,
    depth: usize,
    manifests: &mut Vec<PathBuf>,
) -> Result<(), PackageError> {
    if depth > 4 {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|source| PackageError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| PackageError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| PackageError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(PackageError::UnsafePath(path));
        }
        if metadata.is_dir() {
            collect_manifests(&path, depth + 1, manifests)?;
        } else if entry.file_name() == MANIFEST_NAME {
            manifests.push(path);
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct StagingBudget {
    files: usize,
    bytes: u64,
}

fn inspect_tree(
    root: &Path,
    directory: &Path,
    limits: &StagingLimits,
    budget: &mut StagingBudget,
) -> Result<(), PackageError> {
    for entry in fs::read_dir(directory).map_err(|source| PackageError::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| PackageError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| PackageError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(PackageError::UnsafePath(path));
        }
        if metadata.is_dir() {
            inspect_tree(root, &path, limits, budget)?;
        } else if metadata.is_file() {
            budget.files = budget
                .files
                .checked_add(1)
                .ok_or(PackageError::StagingLimit("file count overflow"))?;
            budget.bytes = budget
                .bytes
                .checked_add(metadata.len())
                .ok_or(PackageError::StagingLimit("byte count overflow"))?;
            if budget.files > limits.maximum_files {
                return Err(PackageError::StagingLimit("maximum files"));
            }
            if metadata.len() > limits.maximum_file_bytes {
                return Err(PackageError::StagingLimit("maximum file bytes"));
            }
            if budget.bytes > limits.maximum_total_bytes {
                return Err(PackageError::StagingLimit("maximum total bytes"));
            }
        } else {
            return Err(PackageError::UnsafePath(path));
        }
        if !path.starts_with(root) {
            return Err(PackageError::UnsafePath(path));
        }
    }
    Ok(())
}

fn resolve_package_file(root: &Path, relative: &str) -> Result<PathBuf, PackageError> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
        })
    {
        return Err(PackageError::UnsafePath(relative.to_path_buf()));
    }

    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(PackageError::UnsafePath(relative.to_path_buf()));
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|source| PackageError::Io {
            path: current.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(PackageError::UnsafePath(current));
        }
    }
    let canonical = fs::canonicalize(&current).map_err(|source| PackageError::Io {
        path: current.clone(),
        source,
    })?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(PackageError::UnsafePath(canonical));
    }
    Ok(canonical)
}

fn read_bounded(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, PackageError> {
    let metadata = fs::metadata(path).map_err(|source| PackageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.len() > maximum_bytes {
        return Err(PackageError::StagingLimit("bounded read"));
    }
    let file = File::open(path).map_err(|source| PackageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| PackageError::StagingLimit("platform address space"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| PackageError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(PackageError::StagingLimit("bounded read"));
    }
    Ok(bytes)
}

fn verify_hash(path: &Path, expected: &str, maximum_bytes: u64) -> Result<(), PackageError> {
    let bytes = read_bounded(path, maximum_bytes)?;
    if digest_bytes(&bytes) != expected.to_ascii_lowercase() {
        return Err(PackageError::HashMismatch {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
