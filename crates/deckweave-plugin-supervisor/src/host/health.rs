// SPDX-License-Identifier: GPL-3.0-or-later
//! Current runtime health, intentionally separate from historical apply receipts.

use deckweave_domain::ids::{ContributionId, PackageId, RuntimeGroupId};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHealthStatus {
    Starting,
    Active,
    Degraded,
    Unavailable,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHealthEntry {
    pub package_id: PackageId,
    pub runtime_group_id: RuntimeGroupId,
    pub contribution_id: ContributionId,
    pub generation: u64,
    pub status: RuntimeHealthStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeHealthSnapshot {
    pub runtimes: Vec<RuntimeHealthEntry>,
}

#[derive(Clone, Default)]
pub(super) struct RuntimeHealthStore {
    entries: Arc<Mutex<BTreeMap<String, RuntimeHealthEntry>>>,
}

impl RuntimeHealthStore {
    pub(super) fn update(&self, entry: RuntimeHealthEntry) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(key(&entry), entry);
        }
    }

    pub(super) fn snapshot(&self) -> RuntimeHealthSnapshot {
        RuntimeHealthSnapshot {
            runtimes: self
                .entries
                .lock()
                .map(|entries| entries.values().cloned().collect())
                .unwrap_or_default(),
        }
    }

    pub(super) fn mark_all_stopped(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            for entry in entries.values_mut() {
                entry.status = RuntimeHealthStatus::Stopped;
                entry.detail = None;
            }
        }
    }
}

fn key(entry: &RuntimeHealthEntry) -> String {
    format!(
        "{}:{}:{}",
        entry.package_id, entry.runtime_group_id, entry.contribution_id
    )
}
