// SPDX-License-Identifier: GPL-3.0-or-later
//! Host planning and cutover port implementations.

use super::{
    config::HostRuntimeError,
    coordinator::{PreparedRuntime, ProductionRuntimeCoordinator},
    instance::elapsed_millis,
};
use deckweave_automation::ExecutionPlanKind;
use deckweave_domain::ids::OperationId;
use deckweave_host::{
    ApplyPlan, ApplyPlanningError, PlanningClock, PlanningEvidence, PlanningEvidenceError,
    PlanningEvidenceSource, PreparedActivation, RuntimeActivation, RuntimeActivationError,
    RuntimeCommitOutcome, RuntimeCutover, RuntimeCutoverError,
};
use deckweave_workspace::{ApplyCommitRecord, WorkspaceRevision};

impl PlanningEvidenceSource for ProductionRuntimeCoordinator {
    fn snapshot(&self) -> Result<PlanningEvidence, PlanningEvidenceError> {
        let registry_revision = self
            .registry_snapshot()
            .map_err(|error| PlanningEvidenceError::Unavailable(error.to_string()))?
            .revision;
        let grant_revision = self
            .authorization
            .revision()
            .map_err(|error| PlanningEvidenceError::Unavailable(error.to_string()))?;
        let active = self
            .operations
            .active_commit()
            .map_err(|error| PlanningEvidenceError::Unavailable(error.to_string()))?
            .map(|commit| commit.candidate);
        Ok(PlanningEvidence {
            host_boot_id: self.host_boot_id.clone(),
            registry_revision,
            grant_revision,
            active,
        })
    }
}

impl PlanningClock for ProductionRuntimeCoordinator {
    fn monotonic_millis(&self) -> Result<u64, ApplyPlanningError> {
        Ok(elapsed_millis(self.clock))
    }
}

impl RuntimeActivation for ProductionRuntimeCoordinator {
    fn prepare(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
    ) -> Result<PreparedActivation, RuntimeActivationError> {
        if self
            .lock_state()
            .map_err(runtime_prepare)?
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.matches(operation_id, plan.token().plan_hash()))
        {
            return Ok(PreparedActivation::new(
                operation_id.clone(),
                plan.token().plan_hash().into(),
            ));
        }
        if self
            .lock_state()
            .map_err(runtime_prepare)?
            .prepared
            .is_some()
        {
            return Err(RuntimeActivationError::PrepareFailed(
                "another prepared runtime exists".into(),
            ));
        }
        let prepared = match plan.compiled().kind() {
            ExecutionPlanKind::EmptyFixture => PreparedRuntime::Empty {
                operation_id: operation_id.clone(),
                plan_hash: plan.token().plan_hash().into(),
            },
            ExecutionPlanKind::VirtualEchoFixture => self
                .prepare_virtual(operation_id, plan)
                .map_err(runtime_prepare)?,
        };
        self.lock_state().map_err(runtime_prepare)?.prepared = Some(prepared);
        Ok(PreparedActivation::new(
            operation_id.clone(),
            plan.token().plan_hash().into(),
        ))
    }
}

impl RuntimeCutover for ProductionRuntimeCoordinator {
    fn enter_barrier(&self, operation_id: &OperationId) -> Result<(), RuntimeCutoverError> {
        let mut state = self.lock_state().map_err(runtime_barrier)?;
        if !state
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.operation_id() == operation_id)
        {
            return Err(RuntimeCutoverError::Barrier(
                "prepared runtime does not match operation".into(),
            ));
        }
        state.dispatcher_closed = true;
        Ok(())
    }

    fn abort_before_commit(&self, operation_id: &OperationId) -> Result<(), RuntimeCutoverError> {
        let prepared = {
            let mut state = self.lock_state().map_err(runtime_abort)?;
            let prepared = state.prepared.take();
            state.dispatcher_closed = false;
            prepared
        };
        if let Some(prepared) = prepared {
            if prepared.operation_id() != operation_id {
                return Err(RuntimeCutoverError::Abort(
                    "prepared runtime belongs to another operation".into(),
                ));
            }
            self.stop_prepared(prepared).map_err(runtime_abort)?;
        }
        Ok(())
    }

    fn activate_committed(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
        prepared: &PreparedActivation,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError> {
        let runtime = self
            .lock_state()
            .map_err(runtime_activation)?
            .prepared
            .take()
            .ok_or_else(|| RuntimeCutoverError::Activation("prepared runtime missing".into()))?;
        if prepared.operation_id() != operation_id
            || !runtime.matches(operation_id, plan.token().plan_hash())
        {
            return Err(RuntimeCutoverError::Activation(
                "prepared activation fence mismatch".into(),
            ));
        }
        self.activate_prepared(runtime).map_err(runtime_activation)
    }

    fn recover_committed(
        &self,
        commit: &ApplyCommitRecord,
        revision: &WorkspaceRevision,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError> {
        self.recover_revision(commit, revision)
            .map_err(|error| RuntimeCutoverError::Recovery(error.to_string()))
    }
}

fn runtime_prepare(error: HostRuntimeError) -> RuntimeActivationError {
    RuntimeActivationError::PrepareFailed(error.to_string())
}

fn runtime_barrier(error: HostRuntimeError) -> RuntimeCutoverError {
    RuntimeCutoverError::Barrier(error.to_string())
}

fn runtime_abort(error: HostRuntimeError) -> RuntimeCutoverError {
    RuntimeCutoverError::Abort(error.to_string())
}

fn runtime_activation(error: HostRuntimeError) -> RuntimeCutoverError {
    RuntimeCutoverError::Activation(error.to_string())
}
