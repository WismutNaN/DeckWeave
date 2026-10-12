// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl LiveExecutionOwner {
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
