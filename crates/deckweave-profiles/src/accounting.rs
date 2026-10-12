// SPDX-License-Identifier: GPL-3.0-or-later
//! Rebuildable accounting; persistent JSON остаётся единственным источником истины.
use super::*;

impl JsonProfileStore {
    pub(crate) fn counted_entries(&self, path: &Path) -> Result<usize, ProfileStoreError> {
        let mut counts = self
            .directory_counts
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        if let Some(count) = counts.get(path) {
            return Ok(*count);
        }
        let count = match fs::read_dir(path) {
            Ok(entries) => {
                let mut count = 0usize;
                for entry in entries {
                    entry.map_err(|source| ProfileStoreError::Io {
                        path: path.into(),
                        source,
                    })?;
                    count = count.checked_add(1).ok_or(ProfileStoreError::TooLarge)?;
                }
                count
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(source) => {
                return Err(ProfileStoreError::Io {
                    path: path.into(),
                    source,
                });
            }
        };
        counts.insert(path.into(), count);
        Ok(count)
    }

    pub(crate) fn account_entry(&self, path: &Path, delta: isize) -> Result<(), ProfileStoreError> {
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        let mut counts = self
            .directory_counts
            .lock()
            .map_err(|_| ProfileStoreError::LockPoisoned)?;
        if let Some(count) = counts.get_mut(parent) {
            *count = count.saturating_add_signed(delta);
        }
        Ok(())
    }

    pub(crate) fn invalidate_count(&self, path: &Path) {
        if let Some(parent) = path.parent()
            && let Ok(mut counts) = self.directory_counts.lock()
        {
            counts.remove(parent);
        }
    }
}
