// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    ActiveWindowState, NativeWindow, WindowAvailability, WindowCatalog, WindowCatalogItem,
    WindowCommandError, WindowFailure, WindowReference, WindowSelector,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt::Write as _, time::Duration};

const MAX_WINDOWS: usize = 256;

pub(crate) trait WindowBackend: Send + 'static {
    fn list_windows(&mut self) -> Result<Vec<NativeWindow>, WindowFailure>;
    fn foreground_token(&mut self) -> Result<Option<u64>, WindowFailure>;
    fn activate(&mut self, token: u64, timeout: Duration) -> Result<(), WindowFailure>;
}

pub(crate) struct WindowService<B: WindowBackend> {
    backend: B,
    generation: u64,
    catalog_revision: u64,
    state_revision: u64,
    records: BTreeMap<String, NativeWindow>,
    catalog: WindowCatalog,
    state: ActiveWindowState,
}

impl<B: WindowBackend> WindowService<B> {
    pub(crate) fn new(backend: B, generation: u64) -> Self {
        let generation = generation.max(1);
        Self {
            backend,
            generation,
            catalog_revision: 0,
            state_revision: 0,
            records: BTreeMap::new(),
            catalog: WindowCatalog {
                generation,
                windows: Vec::new(),
                total: 0,
                next_offset: None,
                revision: 0,
            },
            state: ActiveWindowState {
                availability: WindowAvailability::NoForeground,
                generation,
                window_ref: None,
                process_id: None,
                executable: None,
                title: String::new(),
                fresh: false,
                revision: 0,
            },
        }
    }

    pub(crate) fn refresh(&mut self) -> Result<ActiveWindowState, WindowFailure> {
        self.rebuild_catalog()?;
        let foreground = self.backend.foreground_token()?;
        let next = foreground
            .and_then(|token| {
                self.records
                    .iter()
                    .find(|(_, record)| record.token == token)
            })
            .map_or_else(
                || ActiveWindowState {
                    availability: WindowAvailability::NoForeground,
                    generation: self.generation,
                    window_ref: None,
                    process_id: None,
                    executable: None,
                    title: String::new(),
                    fresh: true,
                    revision: 0,
                },
                |(reference, record)| ActiveWindowState {
                    availability: WindowAvailability::Available,
                    generation: self.generation,
                    window_ref: Some(reference.clone()),
                    process_id: Some(record.process_id),
                    executable: record.executable.clone(),
                    title: record.title.clone(),
                    fresh: true,
                    revision: 0,
                },
            );
        if !same_state(&self.state, &next) {
            self.state_revision = self.state_revision.saturating_add(1);
        }
        self.state = ActiveWindowState {
            revision: self.state_revision,
            ..next
        };
        Ok(self.state.clone())
    }

    pub(crate) fn catalog(&mut self) -> Result<WindowCatalog, WindowFailure> {
        self.rebuild_catalog()?;
        Ok(self.catalog.clone())
    }

    pub(crate) fn activate_reference(
        &mut self,
        target: &WindowReference,
        timeout: Duration,
    ) -> Result<ActiveWindowState, WindowCommandError> {
        if target.generation != self.generation || target.reference.len() != 64 {
            return Err(WindowCommandError::before_effect(
                WindowFailure::StaleReference,
            ));
        }
        self.rebuild_catalog()
            .map_err(WindowCommandError::before_effect)?;
        let record = self
            .records
            .get(&target.reference)
            .cloned()
            .ok_or_else(|| WindowCommandError::before_effect(WindowFailure::StaleReference))?;
        self.activate_record(record, timeout)
    }

    pub(crate) fn focus_selector(
        &mut self,
        selector: &WindowSelector,
        timeout: Duration,
    ) -> Result<ActiveWindowState, WindowCommandError> {
        validate_selector(selector).map_err(WindowCommandError::before_effect)?;
        self.rebuild_catalog()
            .map_err(WindowCommandError::before_effect)?;
        let foreground = self
            .backend
            .foreground_token()
            .map_err(WindowCommandError::before_effect)?;
        let record = self
            .records
            .values()
            .filter(|record| selector_matches(selector, record))
            .min_by_key(|record| (record.token != foreground.unwrap_or_default(), record.token))
            .cloned()
            .ok_or_else(|| WindowCommandError::before_effect(WindowFailure::NoMatchingWindow))?;
        self.activate_record(record, timeout)
    }

    fn activate_record(
        &mut self,
        record: NativeWindow,
        timeout: Duration,
    ) -> Result<ActiveWindowState, WindowCommandError> {
        self.backend
            .activate(record.token, timeout)
            .map_err(WindowCommandError::after_attempt)?;
        let state = self.refresh().map_err(WindowCommandError::after_attempt)?;
        if !state
            .window_ref
            .as_ref()
            .and_then(|reference| self.records.get(reference))
            .is_some_and(|current| same_identity(current, &record))
        {
            return Err(WindowCommandError::after_attempt(
                WindowFailure::ForegroundDenied,
            ));
        }
        Ok(state)
    }

    fn rebuild_catalog(&mut self) -> Result<(), WindowFailure> {
        let mut windows = self.backend.list_windows()?;
        windows.sort_by(|left, right| {
            left.process_id
                .cmp(&right.process_id)
                .then_with(|| left.token.cmp(&right.token))
        });
        windows.truncate(MAX_WINDOWS);
        let mut records = BTreeMap::new();
        let mut items = Vec::with_capacity(windows.len());
        for record in windows {
            let reference = reference_for(self.generation, &record);
            items.push(WindowCatalogItem {
                window_ref: reference.clone(),
                process_id: record.process_id,
                executable: record.executable.clone(),
                title: record.title.clone(),
            });
            records.insert(reference, record);
        }
        if self.catalog.windows != items {
            self.catalog_revision = self.catalog_revision.saturating_add(1);
        }
        self.catalog = WindowCatalog {
            generation: self.generation,
            total: u32::try_from(items.len()).unwrap_or(u32::MAX),
            windows: items,
            next_offset: None,
            revision: self.catalog_revision,
        };
        self.records = records;
        Ok(())
    }
}

fn validate_selector(selector: &WindowSelector) -> Result<(), WindowFailure> {
    if selector.executable.is_empty()
        || selector.executable.len() > 32_768
        || selector
            .title_contains
            .as_ref()
            .is_some_and(|title| title.is_empty() || title.len() > 2_048)
    {
        return Err(WindowFailure::InvalidInput);
    }
    Ok(())
}

fn selector_matches(selector: &WindowSelector, record: &NativeWindow) -> bool {
    let Some(executable) = record.executable.as_ref() else {
        return false;
    };
    if !normalized_path(executable).eq_ignore_ascii_case(&normalized_path(&selector.executable)) {
        return false;
    }
    selector
        .title_contains
        .as_ref()
        .is_none_or(|needle| record.title.to_lowercase().contains(&needle.to_lowercase()))
}

fn normalized_path(value: &str) -> String {
    value.replace('/', "\\")
}

fn reference_for(generation: u64, record: &NativeWindow) -> String {
    let mut hasher = Sha256::new();
    hasher.update(generation.to_le_bytes());
    hasher.update(record.token.to_le_bytes());
    hasher.update(record.process_id.to_le_bytes());
    hasher.update(record.process_started_at.unwrap_or_default().to_le_bytes());
    hasher.update(record.executable.as_deref().unwrap_or_default().as_bytes());
    hasher.update([0]);
    hasher.update(record.title.as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn same_state(left: &ActiveWindowState, right: &ActiveWindowState) -> bool {
    left.availability == right.availability
        && left.generation == right.generation
        && left.window_ref == right.window_ref
        && left.process_id == right.process_id
        && left.executable == right.executable
        && left.title == right.title
        && left.fresh == right.fresh
}

fn same_identity(left: &NativeWindow, right: &NativeWindow) -> bool {
    left.token == right.token
        && left.process_id == right.process_id
        && left.process_started_at == right.process_started_at
        && left.executable == right.executable
}

#[cfg(test)]
mod tests;
