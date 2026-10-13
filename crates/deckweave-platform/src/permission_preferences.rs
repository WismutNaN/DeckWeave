// SPDX-License-Identifier: GPL-3.0-or-later
use crate::PreferenceError;
use deckweave_domain::ids::PackageId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionSelection {
    pub package_id: PackageId,
    pub manifest_sha256: String,
    pub permission_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionPreferences {
    pub revision: u64,
    pub selected: Vec<PermissionSelection>,
}
impl Default for PermissionPreferences {
    fn default() -> Self {
        Self {
            revision: 1,
            selected: vec![],
        }
    }
}
impl PermissionPreferences {
    pub fn validate(&self) -> Result<(), PreferenceError> {
        if self.revision == 0
            || self.selected.len() > 4096
            || self.selected.iter().any(|s| {
                s.manifest_sha256.len() != 64
                    || !s.manifest_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            })
            || self
                .selected
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.selected.len()
        {
            return Err(PreferenceError::Invalid);
        }
        Ok(())
    }
}

pub trait PermissionPreferenceStore: Send + Sync {
    fn load(&self) -> Result<PermissionPreferences, PreferenceError>;
    fn compare_exchange(
        &self,
        expected: u64,
        next: &PermissionPreferences,
    ) -> Result<(), PreferenceError>;
}
