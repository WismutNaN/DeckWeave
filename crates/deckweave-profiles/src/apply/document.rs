// SPDX-License-Identifier: GPL-3.0-or-later
//! Checksummed single-document persistence used by the apply repository adapter.

use super::*;
use serde::de::DeserializeOwned;

const APPLY_DOCUMENT_LIMIT: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredApplyDocument<T> {
    storage_version: u32,
    checksum_sha256: String,
    value: T,
}

impl JsonProfileStore {
    pub(super) fn read_apply_document_with_backup<T>(
        &self,
        path: &Path,
    ) -> Result<T, ApplyRepositoryError>
    where
        T: DeserializeOwned + Serialize,
    {
        self.read_apply_document(path).or_else(|primary| {
            self.read_apply_document(&path.with_extension("json.bak"))
                .map_err(|backup| match backup {
                    ApplyRepositoryError::NotFound => primary,
                    other => other,
                })
        })
    }

    pub(super) fn read_apply_document<T>(&self, path: &Path) -> Result<T, ApplyRepositoryError>
    where
        T: DeserializeOwned + Serialize,
    {
        let bytes = read_bounded(path, APPLY_DOCUMENT_LIMIT).map_err(map_profile_error)?;
        let stored: StoredApplyDocument<T> = serde_json::from_slice(&bytes)
            .map_err(|error| ApplyRepositoryError::InvalidDocument(error.to_string()))?;
        if stored.storage_version != STORAGE_VERSION {
            return Err(ApplyRepositoryError::InvalidDocument(
                "unsupported apply storage version".into(),
            ));
        }
        let value_bytes = canonical_json(&stored.value)
            .map_err(|error| ApplyRepositoryError::InvalidDocument(error.to_string()))?;
        if digest(&value_bytes) != stored.checksum_sha256 {
            return Err(ApplyRepositoryError::InvalidDocument(
                "apply document checksum mismatch".into(),
            ));
        }
        Ok(stored.value)
    }

    pub(super) fn write_apply_document<T>(
        &self,
        path: &Path,
        value: &T,
        backup: Option<&Path>,
    ) -> Result<(), ApplyRepositoryError>
    where
        T: Clone + DeserializeOwned + PartialEq + Serialize,
    {
        let value_bytes = canonical_json(value)
            .map_err(|error| ApplyRepositoryError::InvalidDocument(error.to_string()))?;
        let stored = StoredApplyDocument {
            storage_version: STORAGE_VERSION,
            checksum_sha256: digest(&value_bytes),
            value: value.clone(),
        };
        let bytes = canonical_json(&stored)
            .map_err(|error| ApplyRepositoryError::InvalidDocument(error.to_string()))?;
        match self.atomic_files.write_atomic(path, &bytes, backup) {
            Ok(()) => Ok(()),
            Err(error) => match self.read_apply_document::<T>(path) {
                Ok(actual) if actual == *value => Ok(()),
                _ if error.outcome() == PublicationOutcome::Unknown => {
                    Err(ApplyRepositoryError::PublicationUnknown(error.to_string()))
                }
                _ => Err(ApplyRepositoryError::Unavailable(error.to_string())),
            },
        }
    }

    pub(super) fn apply_operations_directory(&self) -> PathBuf {
        self.root.join("operations/apply")
    }

    fn apply_operation_directory(&self, operation_id: &OperationId) -> PathBuf {
        let name = opaque_key(operation_id.as_str());
        let active = self.apply_operations_directory().join(&name);
        let archive = self.root.join("operations/archive").join(name);
        if !active.exists() && archive.is_dir() {
            archive
        } else {
            active
        }
    }

    pub(super) fn apply_intent_path(&self, operation_id: &OperationId) -> PathBuf {
        self.apply_operation_directory(operation_id)
            .join("intent.json")
    }

    pub(super) fn apply_status_path(&self, operation_id: &OperationId) -> PathBuf {
        self.apply_operation_directory(operation_id)
            .join("status.json")
    }

    pub(super) fn active_commit_path(&self) -> PathBuf {
        self.root.join("active/commit.json")
    }
}
