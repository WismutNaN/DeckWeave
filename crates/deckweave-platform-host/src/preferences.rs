// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::localization::LocaleContext;
use deckweave_platform::{AtomicFileStore, LocalePreferenceStore, PreferenceError};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct JsonLocalePreferenceStore {
    path: PathBuf,
    files: Arc<dyn AtomicFileStore>,
    lock: Mutex<()>,
}
impl JsonLocalePreferenceStore {
    pub fn new(path: PathBuf, files: Arc<dyn AtomicFileStore>) -> Self {
        Self {
            path,
            files,
            lock: Mutex::new(()),
        }
    }
    fn read(&self) -> Result<LocaleContext, PreferenceError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LocaleContext::default());
            }
            Err(e) => return Err(PreferenceError::Unavailable(e.to_string())),
        };
        let mut bytes = Vec::new();
        file.take(4097)
            .read_to_end(&mut bytes)
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        if bytes.len() > 4096 {
            return Err(PreferenceError::Invalid);
        }
        let context: LocaleContext =
            serde_json::from_slice(&bytes).map_err(|_| PreferenceError::Invalid)?;
        context.validate().map_err(|_| PreferenceError::Invalid)?;
        Ok(context)
    }
}
impl LocalePreferenceStore for JsonLocalePreferenceStore {
    fn load(&self) -> Result<LocaleContext, PreferenceError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        self.read()
    }
    fn compare_exchange(&self, expected: u64, next: &LocaleContext) -> Result<(), PreferenceError> {
        next.validate().map_err(|_| PreferenceError::Invalid)?;
        if expected.checked_add(1) != Some(next.revision) {
            return Err(PreferenceError::Invalid);
        }
        let _guard = self
            .lock
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        let current = self.read()?;
        if current == *next {
            return Ok(());
        }
        if current.revision != expected {
            return Err(PreferenceError::Conflict);
        }
        let bytes = serde_json::to_vec(next).map_err(|_| PreferenceError::Invalid)?;
        if let Err(error) = self.files.write_atomic(
            &self.path,
            &bytes,
            Some(&self.path.with_extension("json.bak")),
        ) {
            if self.read().is_ok_and(|actual| actual == *next) {
                return Ok(());
            }
            return Err(PreferenceError::Unavailable(error.to_string()));
        }
        Ok(())
    }
}
