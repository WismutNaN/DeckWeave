// SPDX-License-Identifier: GPL-3.0-or-later

use super::{IngressItem, IngressKind, IngressReliability, SchedulerIntent};
use deckweave_domain::{endpoint::StableEndpointRef, ids::IngressOrdinal};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueAdmission {
    Queued,
    Coalesced,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("ingress queue overflow at ordinal {ordinal:?}")]
pub struct IngressOverflow {
    pub ordinal: IngressOrdinal,
    pub cleanup: SchedulerIntent,
}

#[derive(Debug, Default)]
pub struct IngressQueue {
    reliable: VecDeque<IngressItem>,
    values: BTreeMap<StableEndpointRef, IngressItem>,
}

impl IngressQueue {
    pub fn push(
        &mut self,
        item: IngressItem,
        maximum_items: usize,
    ) -> Result<QueueAdmission, Box<IngressOverflow>> {
        let coalescible_source = match &item.kind {
            IngressKind::Signal {
                source,
                reliability: IngressReliability::Coalescible,
                ..
            } => Some(source.clone()),
            _ => None,
        };
        if let Some(source) = coalescible_source {
            let replacing = self.values.contains_key(&source);
            if !replacing && self.len() >= maximum_items {
                return Err(Box::new(overflow(&item)));
            }
            self.values.insert(source, item);
            return Ok(if replacing {
                QueueAdmission::Coalesced
            } else {
                QueueAdmission::Queued
            });
        }

        self.flush_values();
        if self.len() >= maximum_items {
            return Err(Box::new(overflow(&item)));
        }
        self.reliable.push_back(item);
        Ok(QueueAdmission::Queued)
    }

    pub fn pop(&mut self) -> Option<IngressItem> {
        if self.reliable.is_empty() {
            self.flush_values();
        }
        self.reliable.pop_front()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.reliable.len().saturating_add(self.values.len())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reliable.is_empty() && self.values.is_empty()
    }

    fn flush_values(&mut self) {
        let mut values: Vec<_> = std::mem::take(&mut self.values).into_values().collect();
        values.sort_by_key(|item| item.context.ordinal);
        self.reliable.extend(values);
    }
}

fn overflow(item: &IngressItem) -> IngressOverflow {
    IngressOverflow {
        ordinal: item.context.ordinal,
        cleanup: SchedulerIntent::CleanupAll {
            reason: "reliable ingress overflow".into(),
        },
    }
}
