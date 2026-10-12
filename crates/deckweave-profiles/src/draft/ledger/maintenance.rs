// SPDX-License-Identifier: GPL-3.0-or-later
//! Checkpoint публикуется раньше GC; crash оставляет только лишние immutable files.
use super::*;
use std::collections::BTreeSet;

impl JsonProfileStore {
    /// Maintenance вызывается отдельно от обычной mutation. Revisions/assets/apply
    /// records и claims не удаляются; действующие receipts сохраняют свои snapshots.
    pub fn compact_draft(&self, draft_id: &DraftId) -> Result<usize, DraftMutationError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        self.recover_pending_mutations()?;
        let chain = self.load_draft_chain(draft_id)?;
        if chain.recovered_from_backup {
            self.repair_draft_head(&chain.head)?;
        }
        let checkpoint = DraftTransaction {
            storage_version: STORAGE_VERSION,
            previous_transaction_sha256: None,
            draft: chain.transaction.draft,
            receipt: None,
        };
        let hash = self.write_draft_transaction(&checkpoint)?;
        self.publish_draft_head(&DraftHead {
            storage_version: STORAGE_VERSION,
            draft_id: draft_id.clone(),
            transaction_sha256: hash.clone(),
        })?;
        let mut retained = BTreeSet::from([hash, chain.head.transaction_sha256]);
        let receipts = self.mutation_receipts_directory();
        for path in json_files(&receipts)? {
            let record: MutationLedgerRecord = self.read_ledger_document(&path)?;
            self.validate_ledger_record(&record, Some(&record.key))?;
            if path != self.mutation_receipt_path(&record.key)? {
                return Err(ProfileStoreError::InvalidDocument(
                    "receipt path identity mismatch".into(),
                )
                .into());
            }
            if record.receipt.draft_id != *draft_id {
                continue;
            }
            if self.mutation_epoch_retired(&record.key)? {
                fs::remove_file(&path).map_err(|source| ProfileStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                self.account_entry(&path, -1)?;
            } else {
                let transaction = self.read_transaction(draft_id, &record.transaction_sha256)?;
                self.validate_record_transaction(&record, &transaction)?;
                retained.insert(record.transaction_sha256);
            }
        }
        let directory = self.draft_directory(draft_id).join("transactions");
        let mut removed = 0;
        for path in json_files(&directory)? {
            let hash = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    ProfileStoreError::InvalidDocument("invalid transaction name".into())
                })?;
            if !is_sha256(hash) {
                return Err(
                    ProfileStoreError::InvalidDocument("invalid transaction name".into()).into(),
                );
            }
            if !retained.contains(hash) {
                fs::remove_file(&path).map_err(|source| ProfileStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn json_files(directory: &Path) -> Result<Vec<PathBuf>, ProfileStoreError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(source) => {
            return Err(ProfileStoreError::Io {
                path: directory.into(),
                source,
            });
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| ProfileStoreError::Io {
            path: directory.into(),
            source,
        })?;
        let kind = entry.file_type().map_err(|source| ProfileStoreError::Io {
            path: entry.path(),
            source,
        })?;
        if !kind.is_file() || entry.path().extension() != Some(OsStr::new("json")) {
            return Err(ProfileStoreError::InvalidDocument(
                "unexpected maintenance entry".into(),
            ));
        }
        if paths.len() >= 100_000 {
            return Err(ProfileStoreError::TooLarge);
        }
        paths.push(entry.path());
    }
    Ok(paths)
}
