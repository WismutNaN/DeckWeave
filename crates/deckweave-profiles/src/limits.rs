// SPDX-License-Identifier: GPL-3.0-or-later
use crate::ProfileStoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileStoreLimits {
    pub maximum_revision_bytes: u64,
    pub maximum_asset_bytes: u64,
    pub maximum_draft_transactions: usize,
    pub maximum_mutation_receipts: usize,
    pub maximum_pending_mutations: usize,
    pub dedup_window_seconds: u64,
    pub maximum_export_bytes: u64,
}

impl Default for ProfileStoreLimits {
    fn default() -> Self {
        Self {
            maximum_revision_bytes: 16 * 1024 * 1024,
            maximum_asset_bytes: 128 * 1024 * 1024,
            maximum_draft_transactions: 10_000,
            maximum_mutation_receipts: 100_000,
            maximum_pending_mutations: 128,
            dedup_window_seconds: 7 * 24 * 60 * 60,
            maximum_export_bytes: 512 * 1024 * 1024,
        }
    }
}

pub trait MutationClock: Send + Sync {
    fn unix_seconds(&self) -> Result<u64, ProfileStoreError>;
}

#[derive(Debug, Default)]
pub struct SystemMutationClock;

impl MutationClock for SystemMutationClock {
    fn unix_seconds(&self) -> Result<u64, ProfileStoreError> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .map_err(|error| ProfileStoreError::Clock(error.to_string()))
    }
}
