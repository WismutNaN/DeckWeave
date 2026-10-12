// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded in-memory cache for boot-scoped compiled apply plans.

use crate::ApplyPlan;
use std::collections::{BTreeMap, VecDeque};

pub(super) struct PlanCache {
    plans: BTreeMap<String, ApplyPlan>,
    order: VecDeque<String>,
    maximum: usize,
}

impl PlanCache {
    pub(super) fn new(maximum: usize) -> Self {
        Self {
            plans: BTreeMap::new(),
            order: VecDeque::new(),
            maximum,
        }
    }

    pub(super) fn insert(&mut self, plan: ApplyPlan) {
        let hash = plan.token().plan_hash().to_owned();
        if !self.plans.contains_key(&hash) {
            while self.plans.len() >= self.maximum {
                if let Some(expired) = self.order.pop_front() {
                    self.plans.remove(&expired);
                }
            }
            self.order.push_back(hash.clone());
        }
        self.plans.insert(hash, plan);
    }

    pub(super) fn get(&self, hash: &str) -> Option<ApplyPlan> {
        self.plans.get(hash).cloned()
    }
}
