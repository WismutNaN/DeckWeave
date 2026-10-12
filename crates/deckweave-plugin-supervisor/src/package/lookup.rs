// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact immutable artifact lookup used by apply activation.

use super::{IndexedPackage, PackageId, PackageIndex};

impl PackageIndex {
    pub fn resolve_artifact(
        &self,
        package_id: &PackageId,
        version: &str,
        artifact_sha256: &str,
    ) -> Option<&IndexedPackage> {
        self.packages.values().find(|package| {
            package.package_id == *package_id
                && package.version.to_string() == version
                && package
                    .artifacts
                    .values()
                    .any(|artifact| artifact.sha256 == artifact_sha256)
        })
    }
}
