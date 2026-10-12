// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_domain::ids::{DraftId, DraftVersion, PrincipalId};
use deckweave_workspace::{
    DraftEditError, MutationWindow, preview_draft_mutation as edit_draft, validate_draft,
};
use serde::{Deserialize, Serialize};
#[path = "editor.rs"]
mod editor;
pub use editor::DraftEditor;
mod ledger;
use ledger::MutationLedgerRecord;
mod repository;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationReceipt {
    pub storage_version: u32,
    pub key: MutationKey,
    pub payload_fingerprint: String,
    pub draft_id: DraftId,
    pub before_version: DraftVersion,
    pub after_version: DraftVersion,
    pub document_sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MutationOutcome {
    pub draft: WorkspaceDraft,
    pub receipt: MutationReceipt,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedDraft {
    pub draft: WorkspaceDraft,
    pub recovered_from_backup: bool,
}

#[derive(Debug, Error)]
pub enum DraftMutationError {
    #[error(transparent)]
    Store(#[from] ProfileStoreError),
    #[error("draft version conflict: expected {expected}, current {current}")]
    VersionConflict { expected: u64, current: u64 },
    #[error("idempotency key already belongs to another payload")]
    IdempotencyConflict,
    #[error("idempotency window is unknown or expired")]
    IdempotencyExpired,
    #[error("mutation receipt quota is exhausted")]
    ReceiptQuota,
    #[error("draft command is invalid: {0}")]
    InvalidCommand(String),
    #[error("draft version counter exhausted")]
    CounterExhausted,
    #[error("draft transaction chain exceeds configured bound")]
    HistoryLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftTransaction {
    storage_version: u32,
    previous_transaction_sha256: Option<String>,
    draft: WorkspaceDraft,
    receipt: Option<MutationReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftHead {
    storage_version: u32,
    draft_id: DraftId,
    transaction_sha256: String,
}

struct DraftChainHead {
    head: DraftHead,
    transaction: DraftTransaction,
    recovered_from_backup: bool,
}

impl JsonProfileStore {
    pub fn create_draft(&self, draft: &WorkspaceDraft) -> Result<LoadedDraft, DraftMutationError> {
        validate_draft(draft)
            .map_err(|error| DraftMutationError::InvalidCommand(error.to_string()))?;
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        self.recover_pending_mutations()?;
        let head_path = self.draft_head_path(&draft.draft_id);
        if head_path.exists() {
            let loaded = self.load_draft_chain(&draft.draft_id)?;
            if loaded.transaction.draft == *draft {
                return Ok(LoadedDraft {
                    draft: loaded.transaction.draft,
                    recovered_from_backup: loaded.recovered_from_backup,
                });
            }
            return Err(ProfileStoreError::ImmutableRevisionConflict.into());
        }
        let transaction = DraftTransaction {
            storage_version: STORAGE_VERSION,
            previous_transaction_sha256: None,
            draft: draft.clone(),
            receipt: None,
        };
        let transaction_sha256 = self.write_draft_transaction(&transaction)?;
        self.publish_draft_head(&DraftHead {
            storage_version: STORAGE_VERSION,
            draft_id: draft.draft_id.clone(),
            transaction_sha256,
        })?;
        Ok(LoadedDraft {
            draft: draft.clone(),
            recovered_from_backup: false,
        })
    }

    pub fn load_draft(&self, draft_id: &DraftId) -> Result<LoadedDraft, DraftMutationError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        self.recover_pending_mutations()?;
        let chain = self.load_draft_chain(draft_id)?;
        Ok(LoadedDraft {
            draft: chain.transaction.draft,
            recovered_from_backup: chain.recovered_from_backup,
        })
    }

    pub fn preview_draft_mutation(
        &self,
        draft_id: &DraftId,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<WorkspaceDraft, DraftMutationError> {
        edit_draft(&self.load_draft(draft_id)?.draft, expected_version, command)
            .map_err(map_edit_error)
    }

    pub fn mutate_draft(
        &self,
        draft_id: &DraftId,
        key: MutationKey,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<MutationOutcome, DraftMutationError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        self.recover_pending_mutations()?;
        self.validate_mutation_key(&key)?;
        let fingerprint = mutation_fingerprint(draft_id, expected_version, command)?;
        if let Some(outcome) = self.lookup_mutation(&key, &fingerprint)? {
            return Ok(outcome);
        }
        let mut chain = self.load_draft_chain(draft_id)?;
        if chain.recovered_from_backup {
            self.repair_draft_head(&chain.head)?;
        }
        if chain.transaction.draft.version != expected_version {
            return Err(DraftMutationError::VersionConflict {
                expected: expected_version.get(),
                current: chain.transaction.draft.version.get(),
            });
        }

        let completed = expected_version.get().saturating_sub(1);
        if (completed > 0 && completed.is_multiple_of(self.limits.draft_checkpoint_interval.max(1)))
            || self.counted_entries(&self.mutation_receipts_directory())?
                >= self.limits.maximum_mutation_receipts
        {
            // Perform maintenance before accepting the new command: a failed
            // checkpoint can never turn a committed mutation into an error reply.
            self.compact_draft_locked(draft_id)?;
            chain = self.load_draft_chain(draft_id)?;
        }
        let next = edit_draft(&chain.transaction.draft, expected_version, command)
            .map_err(map_edit_error)?;
        let receipt = MutationReceipt {
            storage_version: STORAGE_VERSION,
            key,
            payload_fingerprint: fingerprint,
            draft_id: draft_id.clone(),
            before_version: expected_version,
            after_version: next.version,
            document_sha256: digest(
                &canonical_json(&next.document).map_err(ProfileStoreError::from)?,
            ),
        };
        let transaction = DraftTransaction {
            storage_version: STORAGE_VERSION,
            previous_transaction_sha256: Some(chain.head.transaction_sha256.clone()),
            draft: next,
            receipt: Some(receipt.clone()),
        };
        let transaction_sha256 = self.write_draft_transaction(&transaction)?;
        let ledger_record = MutationLedgerRecord::new(
            chain.head.transaction_sha256,
            transaction_sha256.clone(),
            receipt.clone(),
        );
        self.begin_mutation(&ledger_record)?;
        self.publish_draft_head(&DraftHead {
            storage_version: STORAGE_VERSION,
            draft_id: draft_id.clone(),
            transaction_sha256,
        })?;
        self.commit_mutation(&ledger_record)?;
        Ok(MutationOutcome {
            draft: transaction.draft,
            receipt,
            replayed: false,
        })
    }

    pub fn open_mutation_window(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<MutationWindow, DraftMutationError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        self.recover_pending_mutations()?;
        self.open_mutation_window_locked(principal_id)
    }

    fn draft_directory(&self, draft_id: &DraftId) -> PathBuf {
        self.root.join("drafts").join(opaque_key(draft_id.as_str()))
    }

    fn draft_head_path(&self, draft_id: &DraftId) -> PathBuf {
        self.draft_directory(draft_id).join("head.json")
    }

    fn transaction_path(&self, draft_id: &DraftId, sha256: &str) -> PathBuf {
        self.draft_directory(draft_id)
            .join("transactions")
            .join(format!("{sha256}.json"))
    }

    fn write_draft_transaction(
        &self,
        transaction: &DraftTransaction,
    ) -> Result<String, DraftMutationError> {
        let bytes = canonical_json(transaction).map_err(ProfileStoreError::from)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.limits.maximum_revision_bytes {
            return Err(ProfileStoreError::TooLarge.into());
        }
        let sha256 = digest(&bytes);
        let path = self.transaction_path(&transaction.draft.draft_id, &sha256);
        if path.exists() {
            if read_bounded(&path, self.limits.maximum_revision_bytes)? != bytes {
                return Err(ProfileStoreError::ChecksumMismatch.into());
            }
        } else {
            self.atomic_files
                .write_atomic(&path, &bytes, None)
                .map_err(ProfileStoreError::from)?;
        }
        Ok(sha256)
    }

    fn publish_draft_head(&self, head: &DraftHead) -> Result<(), DraftMutationError> {
        let target = self.draft_head_path(&head.draft_id);
        let backup = target.with_extension("json.bak");
        let bytes = canonical_json(head).map_err(ProfileStoreError::from)?;
        self.atomic_files
            .write_atomic(&target, &bytes, Some(&backup))
            .map_err(ProfileStoreError::from)?;
        Ok(())
    }

    fn repair_draft_head(&self, head: &DraftHead) -> Result<(), DraftMutationError> {
        let target = self.draft_head_path(&head.draft_id);
        let bytes = canonical_json(head).map_err(ProfileStoreError::from)?;
        self.atomic_files
            .write_atomic(&target, &bytes, None)
            .map_err(ProfileStoreError::from)?;
        Ok(())
    }

    fn load_draft_chain(&self, draft_id: &DraftId) -> Result<DraftChainHead, DraftMutationError> {
        let target = self.draft_head_path(draft_id);
        match self.read_chain_from_head(draft_id, &target) {
            Ok(mut chain) => {
                chain.recovered_from_backup = false;
                Ok(chain)
            }
            Err(primary_error) => {
                let backup = target.with_extension("json.bak");
                match self.read_chain_from_head(draft_id, &backup) {
                    Ok(mut chain) => {
                        chain.recovered_from_backup = true;
                        Ok(chain)
                    }
                    Err(_) => Err(primary_error),
                }
            }
        }
    }

    fn read_chain_from_head(
        &self,
        draft_id: &DraftId,
        path: &Path,
    ) -> Result<DraftChainHead, DraftMutationError> {
        let head: DraftHead = serde_json::from_slice(&read_bounded(path, 1024 * 1024)?)
            .map_err(ProfileStoreError::from)?;
        if head.storage_version != STORAGE_VERSION || head.draft_id != *draft_id {
            return Err(ProfileStoreError::InvalidDocument("invalid draft head".into()).into());
        }
        let transaction = self.read_transaction(draft_id, &head.transaction_sha256)?;
        Ok(DraftChainHead {
            head,
            transaction,
            recovered_from_backup: false,
        })
    }

    fn read_transaction(
        &self,
        draft_id: &DraftId,
        sha256: &str,
    ) -> Result<DraftTransaction, DraftMutationError> {
        if !is_sha256(sha256) {
            return Err(ProfileStoreError::InvalidDocument(
                "invalid transaction checksum reference".into(),
            )
            .into());
        }
        let path = self.transaction_path(draft_id, sha256);
        let bytes = read_bounded(&path, self.limits.maximum_revision_bytes)?;
        if digest(&bytes) != sha256 {
            return Err(ProfileStoreError::ChecksumMismatch.into());
        }
        let transaction: DraftTransaction =
            serde_json::from_slice(&bytes).map_err(ProfileStoreError::from)?;
        if transaction.storage_version != STORAGE_VERSION || transaction.draft.draft_id != *draft_id
        {
            return Err(ProfileStoreError::InvalidDocument(
                "invalid draft transaction identity/version".into(),
            )
            .into());
        }
        validate_draft(&transaction.draft)
            .map_err(|error| DraftMutationError::InvalidCommand(error.to_string()))?;
        Ok(transaction)
    }
}

fn map_edit_error(error: DraftEditError) -> DraftMutationError {
    match error {
        DraftEditError::VersionConflict { expected, current } => {
            DraftMutationError::VersionConflict { expected, current }
        }
        DraftEditError::InvalidCommand(message) => DraftMutationError::InvalidCommand(message),
        DraftEditError::CounterExhausted => DraftMutationError::CounterExhausted,
    }
}

fn mutation_fingerprint(
    draft_id: &DraftId,
    expected_version: DraftVersion,
    command: &DraftCommand,
) -> Result<String, DraftMutationError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Fingerprint<'a> {
        draft_id: &'a DraftId,
        expected_version: DraftVersion,
        command: &'a DraftCommand,
    }
    Ok(digest(
        &canonical_json(&Fingerprint {
            draft_id,
            expected_version,
            command,
        })
        .map_err(ProfileStoreError::from)?,
    ))
}
