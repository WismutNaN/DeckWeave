// SPDX-License-Identifier: GPL-3.0-or-later
//! Server-issued, persisted idempotency windows.

use super::*;
use deckweave_domain::ids::{DedupEpoch, PrincipalId};
use deckweave_workspace::MutationWindow;

const WINDOW_RANDOM_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMutationWindow {
    storage_version: u32,
    window: MutationWindow,
}

impl JsonProfileStore {
    pub(in crate::draft) fn open_mutation_window_locked(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<MutationWindow, DraftMutationError> {
        let now = self.mutation_clock.unix_seconds()?;
        let path = self.mutation_window_path(principal_id);
        if path.exists() {
            let stored: StoredMutationWindow = self.read_ledger_document(&path)?;
            self.validate_stored_window(&stored, principal_id)?;
            if now < stored.window.expires_at_unix_seconds {
                return Ok(stored.window);
            }
        }

        let expires_at_unix_seconds = now
            .checked_add(self.limits.dedup_window_seconds)
            .ok_or(DraftMutationError::CounterExhausted)?;
        let mut random = [0_u8; WINDOW_RANDOM_BYTES];
        getrandom::fill(&mut random)
            .map_err(|error| ProfileStoreError::Clock(format!("random source failed: {error}")))?;
        let epoch = DedupEpoch::try_new(hex(&random))
            .map_err(|error| ProfileStoreError::InvalidDocument(error.to_string()))?;
        let window = MutationWindow {
            principal_id: principal_id.clone(),
            dedup_epoch: epoch,
            issued_at_unix_seconds: now,
            expires_at_unix_seconds,
        };
        let stored = StoredMutationWindow {
            storage_version: STORAGE_VERSION,
            window: window.clone(),
        };
        let bytes = canonical_json(&stored).map_err(ProfileStoreError::from)?;
        self.atomic_files
            .write_atomic(&path, &bytes, Some(&path.with_extension("json.bak")))
            .map_err(ProfileStoreError::from)?;
        Ok(window)
    }

    pub(in crate::draft) fn validate_mutation_key(
        &self,
        key: &MutationKey,
    ) -> Result<(), DraftMutationError> {
        let path = self.mutation_window_path(&key.principal_id);
        let stored: StoredMutationWindow = match self.read_ledger_document(&path) {
            Ok(stored) => stored,
            Err(ProfileStoreError::NotFound) => {
                return Err(DraftMutationError::IdempotencyExpired);
            }
            Err(error) => return Err(error.into()),
        };
        self.validate_stored_window(&stored, &key.principal_id)?;
        let now = self.mutation_clock.unix_seconds()?;
        if stored.window.dedup_epoch != key.dedup_epoch
            || now >= stored.window.expires_at_unix_seconds
        {
            return Err(DraftMutationError::IdempotencyExpired);
        }
        Ok(())
    }

    fn validate_stored_window(
        &self,
        stored: &StoredMutationWindow,
        principal_id: &PrincipalId,
    ) -> Result<(), DraftMutationError> {
        if stored.storage_version != STORAGE_VERSION
            || stored.window.principal_id != *principal_id
            || stored.window.expires_at_unix_seconds <= stored.window.issued_at_unix_seconds
        {
            return Err(ProfileStoreError::InvalidDocument(
                "invalid mutation window identity/version/range".into(),
            )
            .into());
        }
        Ok(())
    }

    fn mutation_window_path(&self, principal_id: &PrincipalId) -> PathBuf {
        self.root
            .join("mutations/windows")
            .join(format!("{}.json", opaque_key(principal_id.as_str())))
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
