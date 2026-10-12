// SPDX-License-Identifier: GPL-3.0-or-later
//! Durable application-command idempotency claims shared by all use cases.

use super::*;
use deckweave_workspace::{
    CommandClaim, CommandClaimOutcome, CommandClaimRepository, DraftRepositoryError,
};

const CLAIM_DOCUMENT_LIMIT: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredCommandClaim {
    storage_version: u32,
    claim: CommandClaim,
}

impl CommandClaimRepository for JsonProfileStore {
    fn claim_command(
        &self,
        claim: &CommandClaim,
    ) -> Result<CommandClaimOutcome, DraftRepositoryError> {
        if !is_sha256(&claim.payload_fingerprint) {
            return Err(DraftRepositoryError::InvalidCommand(
                "command claim fingerprint is not SHA-256".into(),
            ));
        }
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| DraftRepositoryError::Unavailable("store lock is poisoned".into()))?;
        self.validate_claim_window(claim)?;
        let path = self.command_claim_path(&claim.key)?;
        let record = StoredCommandClaim {
            storage_version: STORAGE_VERSION,
            claim: claim.clone(),
        };
        if path.exists() {
            return self.require_same_claim(&path, &record);
        }
        if self
            .counted_entries(&self.command_claims_directory())
            .map_err(|e| DraftRepositoryError::Unavailable(e.to_string()))?
            >= self.limits.maximum_mutation_receipts
        {
            self.compact_claims_locked()
                .map_err(|e| DraftRepositoryError::Unavailable(e.to_string()))?;
            if self
                .counted_entries(&self.command_claims_directory())
                .map_err(|e| DraftRepositoryError::Unavailable(e.to_string()))?
                >= self.limits.maximum_mutation_receipts
            {
                return Err(DraftRepositoryError::ReceiptQuota);
            }
        }
        let bytes = canonical_json(&record)
            .map_err(|error| DraftRepositoryError::Unavailable(error.to_string()))?;
        match self.atomic_files.write_atomic(&path, &bytes, None) {
            Ok(()) => {
                self.account_entry(&path, 1)
                    .map_err(|e| DraftRepositoryError::Unavailable(e.to_string()))?;
                Ok(CommandClaimOutcome { replayed: false })
            }
            Err(_write_error) if path.exists() => {
                self.invalidate_count(&path);
                self.require_same_claim(&path, &record)
            }
            Err(error) => {
                self.invalidate_count(&path);
                Err(DraftRepositoryError::Unavailable(error.to_string()))
            }
        }
    }
}

impl JsonProfileStore {
    pub fn compact_command_claims(&self) -> Result<usize, ProfileStoreError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        self.compact_claims_locked()
    }

    fn compact_claims_locked(&self) -> Result<usize, ProfileStoreError> {
        let pending = self
            .recoverable_operations_locked(self.limits.maximum_apply_operations)
            .map_err(|e| ProfileStoreError::InvalidDocument(e.to_string()))?;
        let directory = self.command_claims_directory();
        if !directory.exists() {
            return Ok(0);
        }
        let entries = fs::read_dir(&directory).map_err(|source| ProfileStoreError::Io {
            path: directory.clone(),
            source,
        })?;
        let mut removed = 0;
        for entry in entries {
            let path = entry
                .map_err(|source| ProfileStoreError::Io {
                    path: directory.clone(),
                    source,
                })?
                .path();
            let stored: StoredCommandClaim =
                serde_json::from_slice(&read_bounded(&path, CLAIM_DOCUMENT_LIMIT)?)?;
            if stored.storage_version != STORAGE_VERSION
                || self
                    .command_claim_path(&stored.claim.key)
                    .map_err(|e| ProfileStoreError::InvalidDocument(e.to_string()))?
                    != path
            {
                return Err(ProfileStoreError::InvalidDocument(
                    "claim identity mismatch".into(),
                ));
            }
            let key = &stored.claim.key;
            let protected = pending.iter().any(|s| {
                s.intent.principal_id == key.principal_id
                    && s.intent.dedup_epoch == key.dedup_epoch
                    && s.intent.idempotency_key == key.idempotency_key
            });
            if !protected
                && self
                    .mutation_epoch_retired(key)
                    .map_err(|e| ProfileStoreError::InvalidDocument(e.to_string()))?
            {
                fs::remove_file(&path).map_err(|source| ProfileStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                self.account_entry(&path, -1)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn validate_claim_window(&self, claim: &CommandClaim) -> Result<(), DraftRepositoryError> {
        self.validate_mutation_key(&claim.key)
            .map_err(map_claim_error)
    }

    fn command_claims_directory(&self) -> PathBuf {
        self.root.join("mutations/claims")
    }

    fn command_claim_path(&self, key: &MutationKey) -> Result<PathBuf, DraftRepositoryError> {
        let bytes = canonical_json(key)
            .map_err(|error| DraftRepositoryError::Unavailable(error.to_string()))?;
        Ok(self
            .command_claims_directory()
            .join(format!("{}.json", digest(&bytes))))
    }

    fn require_same_claim(
        &self,
        path: &Path,
        wanted: &StoredCommandClaim,
    ) -> Result<CommandClaimOutcome, DraftRepositoryError> {
        let bytes = read_bounded(path, CLAIM_DOCUMENT_LIMIT)
            .map_err(|error| DraftRepositoryError::Unavailable(error.to_string()))?;
        let stored: StoredCommandClaim = serde_json::from_slice(&bytes)
            .map_err(|error| DraftRepositoryError::Unavailable(error.to_string()))?;
        if stored.storage_version != STORAGE_VERSION || stored.claim.key != wanted.claim.key {
            return Err(DraftRepositoryError::InvalidCommand(
                "stored command claim identity/version mismatch".into(),
            ));
        }
        if stored.claim.payload_fingerprint != wanted.claim.payload_fingerprint {
            return Err(DraftRepositoryError::IdempotencyConflict);
        }
        Ok(CommandClaimOutcome { replayed: true })
    }
}

fn map_claim_error(error: DraftMutationError) -> DraftRepositoryError {
    match error {
        DraftMutationError::IdempotencyExpired => DraftRepositoryError::IdempotencyExpired,
        DraftMutationError::ReceiptQuota => DraftRepositoryError::ReceiptQuota,
        other => DraftRepositoryError::Unavailable(other.to_string()),
    }
}
