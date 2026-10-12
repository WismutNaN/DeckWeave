// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::ffi::OsStr;

impl JsonProfileStore {
    /// Moves immutable terminal receipts outside the startup scan. Historical query
    /// remains available; pending work and both selected commit heads stay in place.
    pub fn archive_terminal_operations(&self) -> Result<usize, ApplyRepositoryError> {
        let _guard = self.lock_apply_store()?;
        self.archive_terminal_operations_locked()
    }

    pub(super) fn archive_terminal_operations_locked(&self) -> Result<usize, ApplyRepositoryError> {
        let directory = self.apply_operations_directory();
        if !directory.exists() {
            return Ok(0);
        }
        let mut protected = std::collections::BTreeSet::new();
        for path in [
            self.active_commit_path(),
            self.active_commit_path().with_extension("json.bak"),
        ] {
            match self.read_apply_document::<ApplyCommitRecord>(&path) {
                Ok(commit) => {
                    protected.insert(commit.operation_id);
                }
                Err(ApplyRepositoryError::NotFound) => {}
                // A damaged backup is not an excuse to destroy recovery evidence.
                Err(error) => return Err(error),
            }
        }
        let archive = self.root.join("operations/archive");
        fs::create_dir_all(&archive)
            .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
        let root = fs::canonicalize(&self.root)
            .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
        let destination = fs::canonicalize(&archive)
            .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
        if !destination.starts_with(&root) {
            return Err(ApplyRepositoryError::InvalidDocument(
                "archive escapes data root".into(),
            ));
        }
        let entries = fs::read_dir(&directory)
            .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
        let mut archived = 0;
        for entry in entries {
            let entry = entry.map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
            let kind = entry
                .file_type()
                .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
            if !kind.is_dir() || kind.is_symlink() {
                return Err(ApplyRepositoryError::InvalidDocument(
                    "unexpected operation path".into(),
                ));
            }
            let intent: ApplyOperationIntent =
                self.read_apply_document(&entry.path().join("intent.json"))?;
            if entry.file_name() != OsStr::new(&opaque_key(intent.operation_id.as_str())) {
                return Err(ApplyRepositoryError::InvalidDocument(
                    "operation path mismatch".into(),
                ));
            }
            let snapshot = self.operation_inner(&intent.operation_id)?;
            if !snapshot.state.is_terminal() || protected.contains(&intent.operation_id) {
                continue;
            }
            let key = MutationKey {
                principal_id: intent.principal_id,
                dedup_epoch: intent.dedup_epoch,
                idempotency_key: intent.idempotency_key,
            };
            if !self
                .mutation_epoch_retired(&key)
                .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?
            {
                continue;
            }
            let source = fs::canonicalize(entry.path())
                .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
            if !source.starts_with(&root) {
                return Err(ApplyRepositoryError::InvalidDocument(
                    "operation escapes data root".into(),
                ));
            }
            let target = destination.join(entry.file_name());
            if target.exists() {
                return Err(ApplyRepositoryError::InvalidDocument(
                    "duplicate archived operation".into(),
                ));
            }
            fs::rename(&source, &target)
                .map_err(|e| ApplyRepositoryError::Unavailable(e.to_string()))?;
            self.account_entry(&entry.path(), -1)
                .map_err(map_profile_error)?;
            archived += 1;
        }
        Ok(archived)
    }
}
