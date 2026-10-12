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
        if directory_entry_count(&self.command_claims_directory())?
            >= self.limits.maximum_mutation_receipts
        {
            return Err(DraftRepositoryError::ReceiptQuota);
        }
        let bytes = canonical_json(&record)
            .map_err(|error| DraftRepositoryError::Unavailable(error.to_string()))?;
        match self.atomic_files.write_atomic(&path, &bytes, None) {
            Ok(()) => Ok(CommandClaimOutcome { replayed: false }),
            Err(_write_error) if path.exists() => self.require_same_claim(&path, &record),
            Err(error) => Err(DraftRepositoryError::Unavailable(error.to_string())),
        }
    }
}

impl JsonProfileStore {
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

fn directory_entry_count(path: &Path) -> Result<usize, DraftRepositoryError> {
    match fs::read_dir(path) {
        Ok(mut entries) => entries.try_fold(0_usize, |count, entry| {
            entry.map_err(|error| DraftRepositoryError::Unavailable(error.to_string()))?;
            count.checked_add(1).ok_or_else(|| {
                DraftRepositoryError::Unavailable("command claim counter overflow".into())
            })
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(DraftRepositoryError::Unavailable(error.to_string())),
    }
}

fn map_claim_error(error: DraftMutationError) -> DraftRepositoryError {
    match error {
        DraftMutationError::IdempotencyExpired => DraftRepositoryError::IdempotencyExpired,
        DraftMutationError::ReceiptQuota => DraftRepositoryError::ReceiptQuota,
        other => DraftRepositoryError::Unavailable(other.to_string()),
    }
}
