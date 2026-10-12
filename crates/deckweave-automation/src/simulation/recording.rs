// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, capability-free intent recorder used instead of a live dispatcher.

use crate::SchedulerIntent;

pub(super) struct RecordingDispatcher {
    intents: Vec<SchedulerIntent>,
    capacity: usize,
    dropped: u64,
    overloaded: bool,
}

impl RecordingDispatcher {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            intents: Vec::with_capacity(capacity),
            capacity,
            dropped: 0,
            overloaded: false,
        }
    }

    pub(super) fn record(&mut self, intent: SchedulerIntent) -> bool {
        if self.overloaded {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        if self.intents.len() < self.capacity {
            self.intents.push(intent);
            return true;
        }

        self.overloaded = true;
        self.dropped = self.dropped.saturating_add(2);
        self.intents.pop();
        self.intents.push(SchedulerIntent::CleanupAll {
            reason: "simulation recording dispatcher overflow".into(),
        });
        false
    }

    pub(super) fn mark_ingress_overload(&mut self) {
        self.overloaded = true;
    }

    pub(super) fn overloaded(&self) -> bool {
        self.overloaded
    }

    pub(super) fn finish(self) -> (Vec<SchedulerIntent>, u64) {
        (self.intents, self.dropped)
    }
}
