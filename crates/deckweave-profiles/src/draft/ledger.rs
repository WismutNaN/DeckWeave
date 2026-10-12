// SPDX-License-Identifier: GPL-3.0-or-later
//! Persisted mutation windows and the recoverable global idempotency ledger.

use super::*;
use std::{ffi::OsStr, fs};

const LEDGER_DOCUMENT_LIMIT: u64 = 1024 * 1024;
mod maintenance;
mod window;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MutationLedgerRecord {
    storage_version: u32,
    key: MutationKey,
    payload_fingerprint: String,
    previous_transaction_sha256: String,
    transaction_sha256: String,
    receipt: MutationReceipt,
}

impl MutationLedgerRecord {
    pub(super) fn new(
        previous_transaction_sha256: String,
        transaction_sha256: String,
        receipt: MutationReceipt,
    ) -> Self {
        Self {
            storage_version: STORAGE_VERSION,
            key: receipt.key.clone(),
            payload_fingerprint: receipt.payload_fingerprint.clone(),
            previous_transaction_sha256,
            transaction_sha256,
            receipt,
        }
    }
}

impl JsonProfileStore {
    pub(super) fn lookup_mutation(
        &self,
        key: &MutationKey,
        payload_fingerprint: &str,
    ) -> Result<Option<MutationOutcome>, DraftMutationError> {
        let path = self.mutation_receipt_path(key)?;
        let record: MutationLedgerRecord = match self.read_ledger_document(&path) {
            Ok(record) => record,
            Err(ProfileStoreError::NotFound) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        self.validate_ledger_record(&record, Some(key))?;
        if record.payload_fingerprint != payload_fingerprint {
            return Err(DraftMutationError::IdempotencyConflict);
        }
        let transaction =
            self.read_transaction(&record.receipt.draft_id, &record.transaction_sha256)?;
        self.validate_record_transaction(&record, &transaction)?;
        Ok(Some(MutationOutcome {
            draft: transaction.draft,
            receipt: record.receipt,
            replayed: true,
        }))
    }

    pub(super) fn begin_mutation(
        &self,
        record: &MutationLedgerRecord,
    ) -> Result<(), DraftMutationError> {
        self.validate_ledger_record(record, Some(&record.key))?;
        let receipt_path = self.mutation_receipt_path(&record.key)?;
        if receipt_path.exists() {
            return self.require_same_ledger_document(&receipt_path, record);
        }
        if self.counted_entries(&self.mutation_receipts_directory())?
            >= self.limits.maximum_mutation_receipts
        {
            return Err(DraftMutationError::ReceiptQuota);
        }
        let pending_path = self.pending_mutation_path(&record.key)?;
        if !pending_path.exists()
            && self.counted_entries(&self.pending_mutations_directory())?
                >= self.limits.maximum_pending_mutations
        {
            return Err(DraftMutationError::ReceiptQuota);
        }
        self.write_immutable_ledger_document(&pending_path, record)
    }

    pub(super) fn commit_mutation(
        &self,
        record: &MutationLedgerRecord,
    ) -> Result<(), DraftMutationError> {
        let path = self.mutation_receipt_path(&record.key)?;
        self.write_immutable_ledger_document(&path, record)?;
        self.remove_pending_mutation(&record.key)
    }

    pub(super) fn recover_pending_mutations(&self) -> Result<(), DraftMutationError> {
        let directory = self.pending_mutations_directory();
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(ProfileStoreError::Io {
                    path: directory,
                    source,
                }
                .into());
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| ProfileStoreError::Io {
                path: directory.clone(),
                source,
            })?;
            if entry
                .file_type()
                .map_err(|source| ProfileStoreError::Io {
                    path: entry.path(),
                    source,
                })?
                .is_file()
            {
                paths.push(entry.path());
            }
            if paths.len() > self.limits.maximum_pending_mutations {
                return Err(DraftMutationError::ReceiptQuota);
            }
        }
        paths.sort();
        for path in paths {
            if path.extension() != Some(OsStr::new("json")) {
                return Err(ProfileStoreError::InvalidDocument(
                    "unexpected file in pending mutation ledger".into(),
                )
                .into());
            }
            let record: MutationLedgerRecord = self.read_ledger_document(&path)?;
            self.validate_ledger_record(&record, Some(&record.key))?;
            if path != self.pending_mutation_path(&record.key)? {
                return Err(ProfileStoreError::InvalidDocument(
                    "pending mutation path does not match its key".into(),
                )
                .into());
            }
            self.recover_pending_mutation(&record)?;
        }
        Ok(())
    }

    fn recover_pending_mutation(
        &self,
        record: &MutationLedgerRecord,
    ) -> Result<(), DraftMutationError> {
        let receipt_path = self.mutation_receipt_path(&record.key)?;
        if receipt_path.exists() {
            self.require_same_ledger_document(&receipt_path, record)?;
            return self.remove_pending_mutation(&record.key);
        }

        let candidate =
            self.read_transaction(&record.receipt.draft_id, &record.transaction_sha256)?;
        self.validate_record_transaction(record, &candidate)?;
        let chain = self.load_draft_chain(&record.receipt.draft_id)?;
        if chain.head.transaction_sha256 == record.previous_transaction_sha256 {
            self.publish_draft_head(&DraftHead {
                storage_version: STORAGE_VERSION,
                draft_id: record.receipt.draft_id.clone(),
                transaction_sha256: record.transaction_sha256.clone(),
            })?;
        } else if !self.transaction_is_in_chain(
            &record.receipt.draft_id,
            &chain.head.transaction_sha256,
            &record.transaction_sha256,
        )? {
            return Err(ProfileStoreError::InvalidDocument(
                "pending mutation diverges from the published draft chain".into(),
            )
            .into());
        }
        self.commit_mutation(record)
    }

    fn transaction_is_in_chain(
        &self,
        draft_id: &DraftId,
        head_sha256: &str,
        wanted_sha256: &str,
    ) -> Result<bool, DraftMutationError> {
        let mut current = Some(head_sha256.to_owned());
        for _ in 0..self.limits.maximum_draft_transactions {
            let Some(sha256) = current else {
                return Ok(false);
            };
            if sha256 == wanted_sha256 {
                return Ok(true);
            }
            current = self
                .read_transaction(draft_id, &sha256)?
                .previous_transaction_sha256;
        }
        Err(DraftMutationError::HistoryLimit)
    }

    fn validate_ledger_record(
        &self,
        record: &MutationLedgerRecord,
        expected_key: Option<&MutationKey>,
    ) -> Result<(), DraftMutationError> {
        if record.storage_version != STORAGE_VERSION
            || expected_key.is_some_and(|key| *key != record.key)
            || record.receipt.storage_version != STORAGE_VERSION
            || record.receipt.key != record.key
            || record.receipt.payload_fingerprint != record.payload_fingerprint
            || !is_sha256(&record.previous_transaction_sha256)
            || !is_sha256(&record.transaction_sha256)
        {
            return Err(ProfileStoreError::InvalidDocument(
                "invalid mutation ledger record".into(),
            )
            .into());
        }
        Ok(())
    }

    fn validate_record_transaction(
        &self,
        record: &MutationLedgerRecord,
        transaction: &DraftTransaction,
    ) -> Result<(), DraftMutationError> {
        let document_sha256 =
            digest(&canonical_json(&transaction.draft.document).map_err(ProfileStoreError::from)?);
        if transaction.previous_transaction_sha256.as_deref()
            != Some(record.previous_transaction_sha256.as_str())
            || transaction.receipt.as_ref() != Some(&record.receipt)
            || transaction.draft.draft_id != record.receipt.draft_id
            || transaction.draft.version != record.receipt.after_version
            || document_sha256 != record.receipt.document_sha256
        {
            return Err(ProfileStoreError::InvalidDocument(
                "mutation receipt does not match its draft transaction".into(),
            )
            .into());
        }
        Ok(())
    }

    fn mutation_receipts_directory(&self) -> PathBuf {
        self.root.join("mutations/receipts")
    }

    fn pending_mutations_directory(&self) -> PathBuf {
        self.root.join("mutations/pending")
    }

    fn mutation_receipt_path(&self, key: &MutationKey) -> Result<PathBuf, DraftMutationError> {
        Ok(self
            .mutation_receipts_directory()
            .join(format!("{}.json", mutation_key_digest(key)?)))
    }

    fn pending_mutation_path(&self, key: &MutationKey) -> Result<PathBuf, DraftMutationError> {
        Ok(self
            .pending_mutations_directory()
            .join(format!("{}.json", mutation_key_digest(key)?)))
    }

    fn read_ledger_document<T: for<'de> Deserialize<'de>>(
        &self,
        path: &Path,
    ) -> Result<T, ProfileStoreError> {
        serde_json::from_slice(&read_bounded(path, LEDGER_DOCUMENT_LIMIT)?)
            .map_err(ProfileStoreError::from)
    }

    fn write_immutable_ledger_document<T: Serialize + PartialEq>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), DraftMutationError> {
        if path.exists() {
            return self.require_same_ledger_document(path, value);
        }
        let bytes = canonical_json(value).map_err(ProfileStoreError::from)?;
        match self.atomic_files.write_atomic(path, &bytes, None) {
            Ok(()) => {
                self.account_entry(path, 1)?;
                Ok(())
            }
            Err(write_error) if path.exists() => {
                self.invalidate_count(path);
                self.require_same_ledger_document(path, value)
                    .map_err(|_| DraftMutationError::Store(ProfileStoreError::Storage(write_error)))
            }
            Err(error) => {
                self.invalidate_count(path);
                Err(ProfileStoreError::Storage(error).into())
            }
        }
    }

    fn require_same_ledger_document<T: Serialize + PartialEq>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), DraftMutationError> {
        let expected = canonical_json(value).map_err(ProfileStoreError::from)?;
        let actual = read_bounded(path, LEDGER_DOCUMENT_LIMIT)?;
        if actual != expected {
            return Err(ProfileStoreError::ImmutableRevisionConflict.into());
        }
        Ok(())
    }

    fn remove_pending_mutation(&self, key: &MutationKey) -> Result<(), DraftMutationError> {
        let path = self.pending_mutation_path(key)?;
        match fs::remove_file(&path) {
            Ok(()) => {
                self.account_entry(&path, -1)?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ProfileStoreError::Io { path, source }.into()),
        }
    }
}

fn mutation_key_digest(key: &MutationKey) -> Result<String, DraftMutationError> {
    Ok(digest(
        &canonical_json(key).map_err(ProfileStoreError::from)?,
    ))
}
