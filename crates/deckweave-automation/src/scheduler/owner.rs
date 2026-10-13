// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    IngressContext, IngressItem, IngressOverflow, IngressQueue, QueueAdmission, SchedulerFault,
    SchedulerIntent, SchedulerState, SchedulerTransitionError, transition,
};
use crate::{ExecutableGraph, GraphLimits, GraphValidationError};
use deckweave_domain::ids::{ActivationEpoch, HostBootId};

/// The single in-memory owner of one activated workspace's ordered ingress and
/// scheduler state. It performs no I/O; callers execute the returned intents.
#[derive(Debug)]
pub struct WorkspaceScheduler {
    plan: ExecutableGraph,
    state: SchedulerState,
    ingress: IngressQueue,
    limits: GraphLimits,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerStep {
    pub ingress: IngressContext,
    pub intents: Vec<SchedulerIntent>,
    pub faults: Vec<SchedulerFault>,
}

impl WorkspaceScheduler {
    pub fn new(
        plan: ExecutableGraph,
        host_boot_id: HostBootId,
        activation_epoch: ActivationEpoch,
        limits: GraphLimits,
    ) -> Result<Self, GraphValidationError> {
        limits.validate()?;
        let mut state = SchedulerState::new(host_boot_id, activation_epoch);
        for slot in &plan.state_slots {
            if let Some(initial_value) = &slot.initial_value {
                state
                    .values
                    .insert(slot.node_id.clone(), initial_value.clone());
            }
        }
        Ok(Self {
            plan,
            state,
            ingress: IngressQueue::default(),
            limits,
        })
    }

    pub fn admit(&mut self, item: IngressItem) -> Result<QueueAdmission, Box<IngressOverflow>> {
        self.ingress.push(item, self.limits.maximum_queued_events)
    }

    pub fn step(
        &mut self,
        now_ticks: u64,
    ) -> Result<Option<SchedulerStep>, SchedulerTransitionError> {
        let Some(item) = self.ingress.pop() else {
            return Ok(None);
        };
        let ingress = item.context.clone();
        let result = transition(
            &self.plan,
            self.state.clone(),
            item,
            now_ticks,
            &self.limits,
        )?;
        self.state = result.state;
        Ok(Some(SchedulerStep {
            ingress,
            intents: result.intents,
            faults: result.faults,
        }))
    }

    #[must_use]
    pub fn state(&self) -> &SchedulerState {
        &self.state
    }

    #[must_use]
    pub fn queued_items(&self) -> usize {
        self.ingress.len()
    }

    /// Reopens the same plan after a pre-commit apply abort. Pending effects
    /// have already been cancelled by the barrier and are never replayed.
    pub fn reopen_after_barrier(&mut self) {
        if !self.state.invalidated {
            self.state.closed = false;
        }
    }
}
