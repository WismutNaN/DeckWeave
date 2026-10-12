// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_platform::{AtomicFileStore, StorageError};
#[cfg(not(windows))]
use std::fs::File;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeAtomicFileStore;

impl AtomicFileStore for NativeAtomicFileStore {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), StorageError> {
        let parent = target
            .parent()
            .ok_or_else(|| StorageError::Unavailable("target has no parent directory".into()))?;
        fs::create_dir_all(parent)?;
        if let Some(backup) = backup
            && backup.parent() != Some(parent)
        {
            return Err(StorageError::Unavailable(
                "atomic backup must share the target directory".into(),
            ));
        }
        let file_name = target
            .file_name()
            .ok_or_else(|| StorageError::Unavailable("target has no file name".into()))?
            .to_string_lossy();
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        if let Err(error) = (|| -> Result<(), std::io::Error> {
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        })() {
            let _ignored = fs::remove_file(&temporary);
            return Err(error.into());
        }
        drop(file);
        let result = replace_file(target, &temporary, backup);
        if result.is_err() {
            let _ignored = fs::remove_file(&temporary);
        }
        result
    }
}

#[cfg(windows)]
fn replace_file(
    target: &Path,
    replacement: &Path,
    backup: Option<&Path>,
) -> Result<(), StorageError> {
    crate::windows_storage::replace_file(target, replacement, backup)
}

#[cfg(not(windows))]
fn replace_file(
    target: &Path,
    replacement: &Path,
    backup: Option<&Path>,
) -> Result<(), StorageError> {
    if target.exists()
        && let Some(backup) = backup
    {
        fs::copy(target, backup)?;
        File::open(backup)?.sync_all()?;
    }
    fs::rename(replacement, target)?;
    File::open(
        target
            .parent()
            .ok_or_else(|| StorageError::Unavailable("target has no parent".into()))?,
    )?
    .sync_all()?;
    Ok(())
}
