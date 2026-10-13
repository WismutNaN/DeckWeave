// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl LiveExecutionOwner {
    /// Whether a new durable revision describes this exact, still-open owner.
    pub fn can_retain_execution(
        &self,
        graph: Option<&ExecutableGraph>,
        binding: Option<&BoundExecutionGraph>,
    ) -> Result<bool, LiveExecutionError> {
        let state = self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        Ok(matches_execution(&state, graph, binding))
    }

    /// Publish the revision's trace identity without resetting runs or timers.
    pub fn retain_execution(
        &self,
        plan_hash: &str,
        graph: Option<&ExecutableGraph>,
        binding: Option<&BoundExecutionGraph>,
    ) -> Result<(), LiveExecutionError> {
        let _order = self
            .transition_order
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        let state = self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        if !matches_execution(&state, graph, binding) {
            return Err(LiveExecutionError::Scheduler(
                "cannot retain a changed or closed execution".into(),
            ));
        }
        self.trace
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?
            .retain_execution(plan_hash);
        Ok(())
    }

    pub fn activate(
        &self,
        plan_hash: &str,
        graph: Option<&ExecutableGraph>,
        binding: Option<&BoundExecutionGraph>,
        limits: GraphLimits,
    ) -> Result<ActivationEpoch, LiveExecutionError> {
        let _order = self
            .transition_order
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        let next_epoch = state
            .next_epoch
            .checked_add(1)
            .ok_or(LiveExecutionError::EpochExhausted)?;
        let epoch =
            ActivationEpoch::try_new(next_epoch).map_err(|_| LiveExecutionError::EpochExhausted)?;
        let Some(graph) = graph else {
            self.trace
                .lock()
                .map_err(|_| LiveExecutionError::Poisoned)?
                .activate(plan_hash, None, epoch, limits.maximum_trace_entries);
            state.next_epoch = next_epoch;
            state.active = None;
            return Ok(epoch);
        };
        let binding = binding.ok_or(LiveExecutionError::MissingGraph)?;
        let bindings = binding
            .endpoints
            .iter()
            .map(|endpoint| (endpoint.node_id.clone(), endpoint.endpoint.clone()))
            .collect::<BTreeMap<_, _>>();
        for contract in &graph.endpoint_contracts {
            if !bindings.contains_key(&contract.node_id) {
                return Err(LiveExecutionError::MissingBinding(contract.node_id.clone()));
            }
        }
        let scheduler =
            WorkspaceScheduler::new(graph.clone(), self.host_boot_id.clone(), epoch, limits)
                .map_err(|error| LiveExecutionError::Scheduler(error.to_string()))?;
        self.trace
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?
            .activate(plan_hash, Some(graph), epoch, limits.maximum_trace_entries);
        state.next_epoch = next_epoch;
        state.active = Some(ActiveExecution {
            epoch,
            graph: graph.clone(),
            scheduler,
            bindings,
            invocation_routes: BTreeMap::new(),
            next_ordinal: 0,
        });
        Ok(epoch)
    }
}

fn matches_execution(
    state: &OwnerState,
    graph: Option<&ExecutableGraph>,
    binding: Option<&BoundExecutionGraph>,
) -> bool {
    let (Some(active), Some(graph), Some(binding)) = (&state.active, graph, binding) else {
        return false;
    };
    !active.scheduler.state().closed
        && active.graph == *graph
        && binding.endpoints.len() == active.bindings.len()
        && binding
            .endpoints
            .iter()
            .all(|bound| active.bindings.get(&bound.node_id) == Some(&bound.endpoint))
}
