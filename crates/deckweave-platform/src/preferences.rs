// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::localization::LocaleContext;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PreferenceError {
    #[error("preference revision conflict")]
    Conflict,
    #[error("invalid locale preference")]
    Invalid,
    #[error("preference storage unavailable: {0}")]
    Unavailable(String),
}

/// Local host settings, outside portable workspace revisions and plugin data.
pub trait LocalePreferenceStore: Send + Sync {
    fn load(&self) -> Result<LocaleContext, PreferenceError>;
    /// Repeating exactly the already-published value succeeds; stale different values fail.
    fn compare_exchange(
        &self,
        expected_revision: u64,
        next: &LocaleContext,
    ) -> Result<(), PreferenceError>;
}
