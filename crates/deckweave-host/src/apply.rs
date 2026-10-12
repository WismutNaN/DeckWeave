// SPDX-License-Identifier: GPL-3.0-or-later
//! Serialized durable apply coordinator owned by Host Lifecycle.

use crate::{
    ApplyPlan, ApplyPlanRequest, ApplyPlanningError, ApplyPlanningService, PreparedActivation,
};
use deckweave_application::{
    ApplyPlanPreview, ApplyUseCase, ApplyUseCaseError, ExecuteApplyRequest, PlanApplyRequest,
};
use deckweave_automation::CompiledExecutionPlan;
use deckweave_domain::ids::{OperationId, WorkspaceRevisionId};
use deckweave_workspace::{
    ApplyCommitRecord, ApplyOperationIntent, ApplyOperationRepository, ApplyOperationSnapshot,
    ApplyOperationState, ApplyRepositoryError, ApplyRuntimeHealth, DraftRepository,
    DraftRepositoryError, WorkspaceRevision,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use thiserror::Error;

mod cache;
mod recovery;
mod support;
use cache::PlanCache;
use support::{
    compiled_plan_from_commit, durable_plan_value, hash_json, map_draft_error, map_planning_error,
    map_repository_error, operation_id, validate_recovery_plan, validate_recovery_revision,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCommitOutcome {
    Applied,
    Degraded(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeCutoverError {
    #[error("runtime barrier failed: {0}")]
    Barrier(String),
    #[error("exclusive runtime handoff failed: {0}")]
    Handoff(String),
    #[error("runtime abort failed: {0}")]
    Abort(String),
    #[error("runtime activation failed: {0}")]
    Activation(String),
    #[error("runtime recovery failed: {0}")]
    Recovery(String),
}

/// Runtime side of the commit barrier. Implementations keep dispatch closed
/// after `enter_barrier` until either abort or committed activation completes.
pub trait RuntimeCutover: Send + Sync {
    fn enter_barrier(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
        prepared: &PreparedActivation,
    ) -> Result<(), RuntimeCutoverError>;
    fn abort_before_commit(&self, operation_id: &OperationId) -> Result<(), RuntimeCutoverError>;
    fn activate_committed(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
        prepared: &PreparedActivation,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError>;
    fn recover_committed(
        &self,
        commit: &ApplyCommitRecord,
        plan: &CompiledExecutionPlan,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError>;
}

pub struct ApplyCoordinator {
    planning: ApplyPlanningService,
    drafts: Arc<dyn DraftRepository>,
    operations: Arc<dyn ApplyOperationRepository>,
    runtime: Arc<dyn RuntimeCutover>,
    plans: Mutex<PlanCache>,
    execution: Mutex<()>,
}

impl ApplyCoordinator {
    pub fn new(
        planning: ApplyPlanningService,
        drafts: Arc<dyn DraftRepository>,
        operations: Arc<dyn ApplyOperationRepository>,
        runtime: Arc<dyn RuntimeCutover>,
        maximum_cached_plans: usize,
    ) -> Result<Self, ApplyUseCaseError> {
        if maximum_cached_plans == 0 {
            return Err(ApplyUseCaseError::Unavailable(
                "plan cache capacity must be positive".into(),
            ));
        }
        Ok(Self {
            planning,
            drafts,
            operations,
            runtime,
            plans: Mutex::new(PlanCache::new(maximum_cached_plans)),
            execution: Mutex::new(()),
        })
    }

    fn execute_new(
        &self,
        request: &ExecuteApplyRequest,
        operation_id: OperationId,
        payload_sha256: String,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        let plan = self.cached_plan(request)?;
        let candidate = self
            .drafts
            .get_candidate(&request.draft_id)
            .map_err(map_draft_error)?;
        let revision_id = WorkspaceRevisionId::try_new(format!("revision-{operation_id}"))
            .map_err(|error| ApplyUseCaseError::Unavailable(error.to_string()))?;
        let revision = candidate.to_revision(revision_id.clone());
        self.operations
            .save_candidate_revision(&revision)
            .map_err(map_repository_error)?;
        let mut extensions = BTreeMap::new();
        extensions.insert(
            deckweave_automation::COMPILED_PLAN_EXTENSION_ID.into(),
            durable_plan_value(plan.compiled())?,
        );
        let intent = ApplyOperationIntent {
            operation_id: operation_id.clone(),
            principal_id: request.principal_id.clone(),
            dedup_epoch: request.dedup_epoch.clone(),
            idempotency_key: request.idempotency_key.clone(),
            payload_sha256,
            draft_id: request.draft_id.clone(),
            draft_version: request.expected_draft_version,
            plan_hash: request.plan_hash.clone(),
            candidate: deckweave_workspace::ActiveSelection {
                workspace_id: plan.token().workspace_id().clone(),
                revision_id,
            },
            previous_active: request.expected_active.clone(),
            allow_disruption: request.allow_disruption,
            candidate_sha256: plan.token().candidate_sha256().into(),
            package_lock_sha256: plan.token().package_lock_sha256().into(),
            extensions,
        };
        let begun = self
            .operations
            .begin_apply(&intent)
            .map_err(map_repository_error)?;
        if begun.replayed && begun.snapshot.state != ApplyOperationState::Queued {
            return Ok(begun.snapshot);
        }
        let prepared = match self.planning.prepare(&operation_id, &plan) {
            Ok(prepared) => prepared,
            Err(error) => return self.fail_before_commit(&operation_id, error.to_string()),
        };
        if let Err(error) = self.operations.mark_prepared(&operation_id) {
            return self.fail_before_commit(&operation_id, error.to_string());
        }
        if let Err(error) = self.runtime.enter_barrier(&operation_id, &plan, &prepared) {
            return self.fail_before_commit(&operation_id, error.to_string());
        }
        if let Err(error) = self.planning.revalidate_for_barrier(&plan, &prepared) {
            return self.fail_before_commit(&operation_id, error.to_string());
        }
        let commit = ApplyCommitRecord::from_intent(&intent);
        if let Err(error) = self.operations.publish_commit(&commit) {
            return self.reconcile_commit_error(&operation_id, &commit, &plan, &prepared, error);
        }
        let outcome = self
            .runtime
            .activate_committed(&operation_id, &plan, &prepared)
            .unwrap_or_else(|error| RuntimeCommitOutcome::Degraded(error.to_string()));
        self.finish_committed(&operation_id, outcome)
    }

    fn cached_plan(&self, request: &ExecuteApplyRequest) -> Result<ApplyPlan, ApplyUseCaseError> {
        let plan = self
            .plans
            .lock()
            .map_err(|_| ApplyUseCaseError::Unavailable("plan cache lock is poisoned".into()))?
            .get(&request.plan_hash)
            .ok_or_else(|| ApplyUseCaseError::InvalidPlan("plan is unknown or expired".into()))?;
        if plan.token().draft_id() != &request.draft_id
            || plan.token().draft_version() != request.expected_draft_version
            || plan.token().expected_active() != request.expected_active.as_ref()
            || plan.token().allow_disruption() != request.allow_disruption
        {
            return Err(ApplyUseCaseError::InvalidPlan(
                "plan does not match apply preconditions".into(),
            ));
        }
        Ok(plan)
    }

    fn fail_before_commit(
        &self,
        operation_id: &OperationId,
        detail: String,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        if let Err(error) = self.runtime.abort_before_commit(operation_id) {
            let _ignored = self.operations.finish_apply(
                operation_id,
                ApplyOperationState::RecoveryRequired,
                ApplyRuntimeHealth::Inactive,
                Some(error.to_string()),
            );
            return Err(ApplyUseCaseError::RecoveryRequired(error.to_string()));
        }
        self.operations
            .finish_apply(
                operation_id,
                ApplyOperationState::Failed,
                ApplyRuntimeHealth::Inactive,
                Some(detail),
            )
            .map_err(map_repository_error)
    }

    fn reconcile_commit_error(
        &self,
        operation_id: &OperationId,
        wanted: &ApplyCommitRecord,
        plan: &ApplyPlan,
        prepared: &PreparedActivation,
        error: ApplyRepositoryError,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        match self.operations.active_commit() {
            Ok(Some(actual)) if actual == *wanted => {
                let _status_repair = self.operations.publish_commit(wanted);
                let outcome = self
                    .runtime
                    .activate_committed(operation_id, plan, prepared)
                    .unwrap_or_else(|activation| {
                        RuntimeCommitOutcome::Degraded(activation.to_string())
                    });
                self.finish_committed(operation_id, outcome)
            }
            Ok(active)
                if active.as_ref().map(|record| &record.candidate)
                    == wanted.previous_active.as_ref() =>
            {
                self.fail_before_commit(operation_id, error.to_string())
            }
            Ok(_) | Err(_) => Err(ApplyUseCaseError::RecoveryRequired(error.to_string())),
        }
    }

    fn finish_committed(
        &self,
        operation_id: &OperationId,
        outcome: RuntimeCommitOutcome,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        let (health, detail) = match outcome {
            RuntimeCommitOutcome::Applied => (ApplyRuntimeHealth::Applied, None),
            RuntimeCommitOutcome::Degraded(detail) => (ApplyRuntimeHealth::Degraded, Some(detail)),
        };
        self.operations
            .finish_apply(operation_id, ApplyOperationState::Succeeded, health, detail)
            .map_err(map_repository_error)
    }
}

impl ApplyUseCase for ApplyCoordinator {
    fn plan_apply(
        &self,
        request: &PlanApplyRequest,
    ) -> Result<ApplyPlanPreview, ApplyUseCaseError> {
        let plan = self
            .planning
            .plan(&ApplyPlanRequest {
                draft_id: request.draft_id.clone(),
                expected_draft_version: request.expected_draft_version,
                expected_active: request.expected_active.clone(),
                allow_disruption: request.allow_disruption,
            })
            .map_err(map_planning_error)?;
        let preview = ApplyPlanPreview {
            plan_hash: plan.token().plan_hash().into(),
            expires_at_monotonic_millis: plan.token().expires_at_monotonic_millis(),
            draft_id: plan.token().draft_id().clone(),
            draft_version: plan.token().draft_version(),
            requires_disruption: plan.compiled().requires_disruption(),
            disruption_allowed: plan.token().allow_disruption(),
        };
        self.plans
            .lock()
            .map_err(|_| ApplyUseCaseError::Unavailable("plan cache lock is poisoned".into()))?
            .insert(plan);
        Ok(preview)
    }

    fn execute_apply(
        &self,
        request: &ExecuteApplyRequest,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        let window = self
            .drafts
            .open_mutation_window(&request.principal_id)
            .map_err(map_draft_error)?;
        if window.dedup_epoch != request.dedup_epoch {
            return Err(ApplyUseCaseError::IdempotencyExpired);
        }
        let payload_sha256 = hash_json(request)?;
        let operation_id = operation_id(request)?;
        let _execution = self
            .execution
            .lock()
            .map_err(|_| ApplyUseCaseError::Unavailable("apply lock is poisoned".into()))?;
        match self.operations.operation(&operation_id) {
            Ok(snapshot)
                if snapshot.intent.payload_sha256 == payload_sha256
                    && snapshot.state != ApplyOperationState::Queued =>
            {
                return Ok(snapshot);
            }
            Ok(snapshot) if snapshot.intent.payload_sha256 == payload_sha256 => {}
            Ok(_) => return Err(ApplyUseCaseError::IdempotencyConflict),
            Err(ApplyRepositoryError::NotFound) => {}
            Err(error) => return Err(map_repository_error(error)),
        }
        self.execute_new(request, operation_id, payload_sha256)
    }

    fn operation(
        &self,
        operation_id: &OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyUseCaseError> {
        self.operations
            .operation(operation_id)
            .map_err(map_repository_error)
    }
}
