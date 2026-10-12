// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure presentation state; no rendering, network reads, or implicit read receipts.
use deckweave_domain::ids::{ActivationEpoch, PackageId, PluginInstanceId, SessionGeneration};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemRef {
    pub package_id: PackageId,
    pub instance_id: PluginInstanceId,
    pub collection: String,
    pub item_id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationFence {
    pub activation_epoch: ActivationEpoch,
    pub provider_generation: SessionGeneration,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPresentation {
    pub fence: PresentationFence,
    pub items: Vec<ItemRef>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PresentationError {
    #[error("invalid or oversized presentation")]
    Invalid,
    #[error("stale presentation or item")]
    Stale,
    #[error("slot or widget is absent")]
    Missing,
    #[error("view/slot capacity exceeded")]
    Capacity,
}

impl CollectionPresentation {
    pub fn validate(&self) -> Result<(), PresentationError> {
        let mut ids = BTreeSet::new();
        if self.fence.revision == 0
            || self.items.len() > 100
            || self.next_cursor.as_ref().is_some_and(|c| c.len() > 4096)
            || self.items.iter().any(|i| {
                [&i.collection, &i.item_id, &i.revision]
                    .iter()
                    .any(|s| s.is_empty() || s.len() > 512)
                    || !ids.insert((&i.package_id, &i.instance_id, &i.collection, &i.item_id))
            })
        {
            return Err(PresentationError::Invalid);
        }
        Ok(())
    }

    /// Capture on press; release/cancel uses this value even after list reorder.
    pub fn hit(
        &self,
        fence: &PresentationFence,
        index: usize,
    ) -> Result<ItemRef, PresentationError> {
        self.validate()?;
        if &self.fence != fence {
            return Err(PresentationError::Stale);
        }
        self.items
            .get(index)
            .cloned()
            .ok_or(PresentationError::Missing)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlotDefinition {
    pub widgets: Vec<String>,
}

#[derive(Default)]
pub struct SlotSelections {
    slots: BTreeMap<String, (SlotDefinition, usize, Vec<usize>)>,
}

impl SlotSelections {
    pub fn define(
        &mut self,
        slot: String,
        definition: SlotDefinition,
    ) -> Result<(), PresentationError> {
        if slot.is_empty()
            || slot.len() > 256
            || definition.widgets.is_empty()
            || definition.widgets.len() > 64
            || definition
                .widgets
                .iter()
                .any(|w| w.is_empty() || w.len() > 256)
            || definition.widgets.iter().collect::<BTreeSet<_>>().len() != definition.widgets.len()
        {
            return Err(PresentationError::Invalid);
        }
        if !self.slots.contains_key(&slot) && self.slots.len() >= 4096 {
            return Err(PresentationError::Capacity);
        }
        self.slots.insert(slot, (definition, 0, Vec::new()));
        Ok(())
    }
    pub fn select(&mut self, slot: &str, widget: &str) -> Result<(), PresentationError> {
        let (definition, selected, history) =
            self.slots.get_mut(slot).ok_or(PresentationError::Missing)?;
        let index = definition
            .widgets
            .iter()
            .position(|w| w == widget)
            .ok_or(PresentationError::Missing)?;
        if index != *selected {
            if history.len() == 64 {
                history.remove(0);
            }
            history.push(*selected);
            *selected = index;
        }
        Ok(())
    }
    pub fn next(&mut self, slot: &str) -> Result<(), PresentationError> {
        let (definition, index, _) = self.slots.get(slot).ok_or(PresentationError::Missing)?;
        let next = definition.widgets[(index + 1) % definition.widgets.len()].clone();
        self.select(slot, &next)
    }
    pub fn back(&mut self, slot: &str) -> Result<(), PresentationError> {
        let (_, selected, history) = self.slots.get_mut(slot).ok_or(PresentationError::Missing)?;
        if let Some(index) = history.pop() {
            *selected = index;
        }
        Ok(())
    }
    pub fn consumers(&self, widget: &str) -> usize {
        self.slots
            .values()
            .filter(|(definition, index, _)| definition.widgets[*index] == widget)
            .count()
    }
    pub fn remove(&mut self, slot: &str) {
        self.slots.remove(slot);
    }
}

/// A persistent view is leased by workspace/instance, never by a hanging invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopViewSession {
    pub view_id: String,
    pub instance_id: PluginInstanceId,
    pub fence: PresentationFence,
    pub lease_expires_host_ticks: u64,
    pub document_id: Option<String>,
}

impl DesktopViewSession {
    pub fn accepts(&self, fence: &PresentationFence, now: u64) -> bool {
        &self.fence == fence && now < self.lease_expires_host_ticks
    }
}
