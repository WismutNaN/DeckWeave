// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact immutable artifact lookup used by apply activation.

use super::*;

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

impl PackageIndex {
    pub fn resolve_artifact(
        &self,
        package_id: &PackageId,
        version: &str,
        artifact_sha256: &str,
    ) -> Option<&IndexedPackage> {
        let mut matching = self.packages.values().filter(|package| {
            package.package_id == *package_id
                && package.version.to_string() == version
                && package
                    .artifacts
                    .values()
                    .any(|artifact| artifact.sha256 == artifact_sha256)
        });
        let found = matching.next()?;
        matching.next().is_none().then_some(found)
    }
}
