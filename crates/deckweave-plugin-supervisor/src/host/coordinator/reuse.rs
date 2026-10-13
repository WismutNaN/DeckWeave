// SPDX-License-Identifier: GPL-3.0-or-later
//! Retain live execution for revisions that change no executable behavior.
use super::*;
use crate::host::health::RuntimeHealthStatus;
use deckweave_domain::ids::GrantRevision;

impl ProductionRuntimeCoordinator {
    pub(in crate::host) fn prepared_reuses_active(
        &self,
        operation_id: &OperationId,
    ) -> Result<bool, HostRuntimeError> {
        Ok(self
            .lock_state()?
            .prepared
            .as_ref()
            .is_some_and(|prepared| {
                prepared.operation_id == *operation_id && prepared.reuse_active
            }))
    }

    pub(super) fn prepare_reuse(
        &self,
        operation_id: &OperationId,
        plan_hash: &str,
        plan: &CompiledExecutionPlan,
        grant_revision: GrantRevision,
    ) -> Result<Option<PreparedRuntime>, HostRuntimeError> {
        let state = self.lock_state()?;
        if !self.can_reuse(&state, plan, grant_revision)? {
            return Ok(None);
        }
        Ok(Some(PreparedRuntime {
            operation_id: operation_id.clone(),
            plan_hash: plan_hash.into(),
            instances: vec![],
            bound_graph: state.active_binding.clone(),
            executable_graph: plan.executable_graph().cloned(),
            compiled: plan.clone(),
            grant_revision,
            reuse_active: true,
        }))
    }

    pub(super) fn can_reuse(
        &self,
        state: &RuntimeState,
        plan: &CompiledExecutionPlan,
        grant_revision: GrantRevision,
    ) -> Result<bool, HostRuntimeError> {
        if state.dispatcher_closed
            || state.active_grant_revision != Some(grant_revision)
            || self.authorization.revision()? != grant_revision
            || !state
                .active_plan
                .as_ref()
                .is_some_and(|active| equivalent(active, plan))
        {
            return Ok(false);
        }
        let health = self.health.snapshot();
        for worker in &state.active {
            let key = worker.key();
            if !worker.is_alive() || worker.grant_revision() != grant_revision {
                return Ok(false);
            }
            for requirement in plan.runtime_requirements().iter().filter(|requirement| {
                requirement.package_id == key.package_id
                    && requirement.runtime_group_id == key.runtime_group_id
            }) {
                if !health.runtimes.iter().any(|entry| {
                    entry.package_id == key.package_id
                        && entry.runtime_group_id == key.runtime_group_id
                        && entry.contribution_id == requirement.contribution_id
                        && entry.generation == key.generation.get()
                        && entry.status == RuntimeHealthStatus::Active
                }) {
                    return Ok(false);
                }
            }
        }
        let Some(binding) = &state.active_binding else {
            return Ok(false);
        };
        let registry = self
            .registry
            .lock()
            .map_err(|_| HostRuntimeError::State("registry lock is poisoned".into()))?;
        for bound in &binding.endpoints {
            if registry.resolve(&bound.endpoint.stable).as_ref() != Ok(&bound.endpoint) {
                return Ok(false);
            }
        }
        self.execution
            .can_retain_execution(plan.executable_graph(), Some(binding))
            .map_err(|error| HostRuntimeError::State(error.to_string()))
    }

    pub(super) fn activate_reuse(
        &self,
        prepared: PreparedRuntime,
    ) -> Result<RuntimeCommitOutcome, HostRuntimeError> {
        let mut state = self.lock_state()?;
        if !self.can_reuse(&state, &prepared.compiled, prepared.grant_revision)? {
            return Err(HostRuntimeError::State(
                "active runtime changed during commit".into(),
            ));
        }
        self.execution
            .retain_execution(
                &prepared.plan_hash,
                prepared.executable_graph.as_ref(),
                prepared.bound_graph.as_ref(),
            )
            .map_err(|error| HostRuntimeError::State(error.to_string()))?;
        state.active_plan = Some(prepared.compiled);
        Ok(RuntimeCommitOutcome::Applied)
    }
}

fn equivalent(active: &CompiledExecutionPlan, next: &CompiledExecutionPlan) -> bool {
    // The semantic digest also includes planning registry/grant fences, which
    // legitimately change after initial activation. Compare executable inputs
    // directly; authorization, live descriptors and health are revalidated above.
    active.executable_graph().is_some()
        && active.executable_graph() == next.executable_graph()
        && active.compiler_id() == next.compiler_id()
        && active.compiler_version() == next.compiler_version()
        && active.runtime_requirements() == next.runtime_requirements()
        // CredentialRef does not identify the resolved secret revision yet.
        // A reapply must still deliver a rotated credential to its instance.
        && next.runtime_requirements().iter().all(|requirement| {
            requirement.configuration.as_ref().is_none_or(|settings| settings.credentials.is_empty())
        })
        && !active.requires_disruption()
        && !next.requires_disruption()
}
