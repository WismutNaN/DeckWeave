// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_platform::{
    AtomicFileStore, PermissionPreferenceStore, PermissionPreferences, PreferenceError,
};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct JsonPermissionPreferenceStore {
    path: PathBuf,
    files: Arc<dyn AtomicFileStore>,
    order: Mutex<()>,
}
impl JsonPermissionPreferenceStore {
    pub fn new(path: PathBuf, files: Arc<dyn AtomicFileStore>) -> Self {
        Self {
            path,
            files,
            order: Mutex::new(()),
        }
    }
    fn read(&self) -> Result<PermissionPreferences, PreferenceError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
            Err(e) => return Err(PreferenceError::Unavailable(e.to_string())),
        };
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        if bytes.len() > 1024 * 1024 {
            return Err(PreferenceError::Invalid);
        }
        let value: PermissionPreferences =
            serde_json::from_slice(&bytes).map_err(|_| PreferenceError::Invalid)?;
        value.validate()?;
        Ok(value)
    }
}
impl PermissionPreferenceStore for JsonPermissionPreferenceStore {
    fn load(&self) -> Result<PermissionPreferences, PreferenceError> {
        let _guard = self
            .order
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        self.read()
    }
    fn compare_exchange(
        &self,
        expected: u64,
        next: &PermissionPreferences,
    ) -> Result<(), PreferenceError> {
        next.validate()?;
        if next.revision != expected.checked_add(1).ok_or(PreferenceError::Invalid)? {
            return Err(PreferenceError::Invalid);
        }
        let _guard = self
            .order
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
