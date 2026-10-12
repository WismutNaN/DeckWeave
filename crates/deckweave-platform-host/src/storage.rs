// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_platform::{AtomicFileStore, PublicationError, StorageError};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
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
    ) -> Result<(), PublicationError> {
        self.write_with(target, bytes, backup, replace_file)
    }
}

impl NativeAtomicFileStore {
    fn write_with<F>(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
        publish: F,
    ) -> Result<(), PublicationError>
    where
        F: FnOnce(&Path, &Path, Option<&Path>) -> Result<(), StorageError>,
    {
        let parent = target
            .parent()
            .ok_or_else(|| StorageError::Unavailable("target has no parent directory".into()))
            .map_err(PublicationError::not_published)?;
        fs::create_dir_all(parent).map_err(PublicationError::not_published)?;
        if let Some(backup) = backup
            && backup.parent() != Some(parent)
        {
            return Err(PublicationError::not_published(StorageError::Unavailable(
                "atomic backup must share the target directory".into(),
            )));
        }
        let file_name = target
            .file_name()
            .ok_or_else(|| StorageError::Unavailable("target has no file name".into()))
            .map_err(PublicationError::not_published)?
            .to_string_lossy();
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{file_name}.tmp-{}-{sequence}",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(PublicationError::not_published)?;
        if let Err(error) = (|| -> Result<(), std::io::Error> {
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        })() {
            let _ignored = fs::remove_file(&temporary);
            return Err(PublicationError::not_published(error));
        }
        drop(file);

        let publish_error = match publish(target, &temporary, backup) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        match target_matches(target, bytes) {
            Ok(true) => {
                let _ignored = fs::remove_file(&temporary);
                Ok(())
            }
            Ok(false) => {
                let _ignored = fs::remove_file(&temporary);
                Err(PublicationError::not_published(publish_error))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let _ignored = fs::remove_file(&temporary);
                Err(PublicationError::not_published(publish_error))
            }
            Err(reconciliation_error) => Err(PublicationError::unknown(
                target.to_path_buf(),
                temporary,
                publish_error,
                reconciliation_error.to_string(),
            )),
        }
    }
}

fn target_matches(target: &Path, expected: &[u8]) -> Result<bool, std::io::Error> {
    let metadata = fs::metadata(target)?;
    if !metadata.is_file() {
        return Err(std::io::Error::other("publication target is not a file"));
    }
    if metadata.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX) {
        return Ok(false);
    }
    let mut file = File::open(target)?;
    let mut offset = 0;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(offset == expected.len());
        }
        if expected.get(offset..offset + read) != Some(&buffer[..read]) {
            return Ok(false);
        }
        offset += read;
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

#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_platform::PublicationOutcome;

    #[test]
    fn error_after_replace_is_reconciled_as_success() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("profile.json");
        fs::write(&target, b"old")?;

        NativeAtomicFileStore.write_with(&target, b"new", None, |target, staged, _| {
            fs::remove_file(target)?;
            fs::rename(staged, target)?;
            Err(StorageError::Unavailable("simulated lost reply".into()))
        })?;

        assert_eq!(fs::read(target)?, b"new");
        assert!(no_staged_files(directory.path())?);
        Ok(())
    }

    #[test]
    fn error_before_replace_is_known_not_published() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("profile.json");
        fs::write(&target, b"old")?;

        let error =
            match NativeAtomicFileStore.write_with(&target, b"new", None, |_target, _staged, _| {
                Err(StorageError::Unavailable(
                    "simulated pre-replace failure".into(),
                ))
            }) {
                Err(error) => error,
                Ok(()) => {
                    return Err(std::io::Error::other("publication unexpectedly succeeded").into());
                }
            };

        assert_eq!(error.outcome(), PublicationOutcome::NotPublished);
        assert_eq!(fs::read(target)?, b"old");
        assert!(no_staged_files(directory.path())?);
        Ok(())
    }

    #[test]
    fn unreadable_target_keeps_staged_bytes_and_reports_unknown()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("profile.json");

        let error =
            match NativeAtomicFileStore.write_with(&target, b"new", None, |target, _staged, _| {
                fs::create_dir(target)?;
                Err(StorageError::Unavailable(
                    "simulated ambiguous failure".into(),
                ))
            }) {
                Err(error) => error,
                Ok(()) => {
                    return Err(std::io::Error::other("publication unexpectedly succeeded").into());
                }
            };

        assert_eq!(error.outcome(), PublicationOutcome::Unknown);
        assert!(error.staged_path().is_some_and(Path::exists));
        Ok(())
    }

    fn no_staged_files(directory: &Path) -> Result<bool, std::io::Error> {
        Ok(fs::read_dir(directory)?
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp-")))
    }
}
