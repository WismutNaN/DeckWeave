// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::permissions::{PermissionCatalogSource, PermissionOffer};
use deckweave_domain::{ids::PrincipalId, security::PermissionRequest};
use deckweave_platform::PermissionSelection;

impl PermissionCatalogSource for PackageIndex {
    fn permission_offers(&self, principal: &PrincipalId) -> Result<Vec<PermissionOffer>, String> {
        let mut offers = Vec::new();
        for package in self.packages() {
            if !package.artifacts.values().any(|a| {
                a.operating_system == std::env::consts::OS
                    && a.architecture == std::env::consts::ARCH
            }) {
                continue;
            }
            for (index, permission) in package.requested_permissions.iter().enumerate() {
                if offers.len() >= 4096 {
                    return Err("permission catalog budget".into());
                }
                offers.push(PermissionOffer {
                    selection: PermissionSelection {
                        package_id: package.package_id.clone(),
                        manifest_sha256: package.manifest_sha256.clone(),
                        permission_index: u32::try_from(index).map_err(|_| "permission index")?,
                    },
                    request: PermissionRequest {
                        principal_id: principal.clone(),
                        package_id: package.package_id.clone(),
                        contribution_id: None,
                        scope: permission.scope.clone(),
                        resource_selector: permission.resource_selector.clone(),
                        access_mode: permission.access_mode,
                        reason: permission.reason.clone(),
                        extensions: Default::default(),
                    },
                    runtime_group_ids: permission.runtime_group_ids.clone(),
                });
            }
        }
        Ok(offers)
    }
}
