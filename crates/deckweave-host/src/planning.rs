// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-owned fencing between immutable candidates, compilation and activation.

use deckweave_automation::{
    CompileFence, CompiledExecutionPlan, PlanCompileError, PlanCompiler, RecoveryCompileInput,
};
use deckweave_domain::ids::{
    DraftId, DraftVersion, GrantRevision, HostBootId, OperationId, RegistryRevision, WorkspaceId,
};
use deckweave_workspace::{
    ActiveSelection, DraftRepository, DraftRepositoryError, WorkspaceCandidate, WorkspaceRevision,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

mod token;
use token::{build_token, calculate_plan_hash};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningEvidence {
    pub host_boot_id: HostBootId,
    pub registry_revision: RegistryRevision,
    pub grant_revision: GrantRevision,
    pub active: Option<ActiveSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyPlanRequest {
    pub draft_id: DraftId,
    pub expected_draft_version: DraftVersion,
    pub expected_active: Option<ActiveSelection>,
    pub allow_disruption: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanToken {
    host_boot_id: HostBootId,
    draft_id: DraftId,
    workspace_id: WorkspaceId,
    draft_version: DraftVersion,
    candidate_sha256: String,
    package_lock_sha256: String,
    assets_sha256: String,
    compiled_plan_sha256: String,
    registry_revision: RegistryRevision,
    grant_revision: GrantRevision,
    expected_active: Option<ActiveSelection>,
    allow_disruption: bool,
    expires_at_monotonic_millis: u64,
    plan_hash: String,
}

impl PlanToken {
    #[must_use]
    pub fn plan_hash(&self) -> &str {
        &self.plan_hash
    }

    #[must_use]
    pub fn expires_at_monotonic_millis(&self) -> u64 {
        self.expires_at_monotonic_millis
    }

    #[must_use]
    pub fn draft_id(&self) -> &DraftId {
        &self.draft_id
    }

    #[must_use]
    pub fn draft_version(&self) -> DraftVersion {
        self.draft_version
    }

    #[must_use]
    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }

    #[must_use]
    pub fn candidate_sha256(&self) -> &str {
        &self.candidate_sha256
    }

    #[must_use]
    pub fn package_lock_sha256(&self) -> &str {
        &self.package_lock_sha256
    }

    #[must_use]
    pub fn expected_active(&self) -> Option<&ActiveSelection> {
        self.expected_active.as_ref()
    }

    #[must_use]
    pub fn allow_disruption(&self) -> bool {
        self.allow_disruption
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyPlan {
    token: PlanToken,
    compiled: CompiledExecutionPlan,
}

impl ApplyPlan {
    #[must_use]
    pub fn token(&self) -> &PlanToken {
        &self.token
    }

    #[must_use]
    pub fn compiled(&self) -> &CompiledExecutionPlan {
        &self.compiled
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedActivation {
    operation_id: OperationId,
    plan_hash: String,
}

impl PreparedActivation {
    #[must_use]
    pub fn new(operation_id: OperationId, plan_hash: String) -> Self {
        Self {
            operation_id,
            plan_hash,
        }
    }

    #[must_use]
    pub fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanPrecondition {
    PlanIntegrity,
    HostBoot,
    Expired,
    DraftVersion,
    CandidateContent,
    PackageLock,
    Assets,
    ActiveSelection,
    RegistryRevision,
    GrantRevision,
    PreparedActivation,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanningEvidenceError {
    #[error("planning evidence is unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeActivationError {
    #[error("runtime activation prepare failed: {0}")]
    PrepareFailed(String),
}

#[derive(Debug, Error)]
pub enum ApplyPlanningError {
    #[error(transparent)]
    Repository(#[from] DraftRepositoryError),
    #[error(transparent)]
    Compile(#[from] PlanCompileError),
    #[error(transparent)]
    Evidence(#[from] PlanningEvidenceError),
    #[error(transparent)]
    Activation(#[from] RuntimeActivationError),
    #[error("planning clock is unavailable: {0}")]
    Clock(String),
    #[error("plan serialization failed: {0}")]
    Serialization(String),
    #[error("plan TTL must be positive")]
    InvalidTtl,
    #[error("plan expiry counter exhausted")]
    CounterExhausted,
    #[error("apply precondition changed: {0:?}")]
    PreconditionChanged(PlanPrecondition),
    #[error("apply plan requires an explicitly allowed disruptive handoff")]
    DisruptionNotAllowed,
}

pub trait PlanningEvidenceSource: Send + Sync {
    fn snapshot(&self) -> Result<PlanningEvidence, PlanningEvidenceError>;
}

pub trait PlanningClock: Send + Sync {
    fn monotonic_millis(&self) -> Result<u64, ApplyPlanningError>;
}

pub trait RuntimeActivation: Send + Sync {
    fn prepare(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
    ) -> Result<PreparedActivation, RuntimeActivationError>;
}

pub struct ApplyPlanningService {
    drafts: Arc<dyn DraftRepository>,
    compiler: Arc<dyn PlanCompiler>,
    evidence: Arc<dyn PlanningEvidenceSource>,
    activation: Arc<dyn RuntimeActivation>,
    clock: Arc<dyn PlanningClock>,
    plan_ttl_millis: u64,
}

impl ApplyPlanningService {
    pub fn new(
        drafts: Arc<dyn DraftRepository>,
        compiler: Arc<dyn PlanCompiler>,
        evidence: Arc<dyn PlanningEvidenceSource>,
        activation: Arc<dyn RuntimeActivation>,
        clock: Arc<dyn PlanningClock>,
        plan_ttl_millis: u64,
    ) -> Result<Self, ApplyPlanningError> {
        if plan_ttl_millis == 0 {
            return Err(ApplyPlanningError::InvalidTtl);
        }
        Ok(Self {
            drafts,
            compiler,
            evidence,
            activation,
            clock,
            plan_ttl_millis,
        })
    }

    pub fn plan(&self, request: &ApplyPlanRequest) -> Result<ApplyPlan, ApplyPlanningError> {
        let candidate = self.drafts.get_candidate(&request.draft_id)?;
        if candidate.version() != request.expected_draft_version {
            return Err(ApplyPlanningError::PreconditionChanged(
                PlanPrecondition::DraftVersion,
            ));
        }
        let evidence = self.evidence.snapshot()?;
        if evidence.active != request.expected_active {
            return Err(ApplyPlanningError::PreconditionChanged(
                PlanPrecondition::ActiveSelection,
            ));
        }
        let now = self.clock.monotonic_millis()?;
        let expires_at_monotonic_millis = now
            .checked_add(self.plan_ttl_millis)
            .ok_or(ApplyPlanningError::CounterExhausted)?;
        let compiled = self.compiler.compile(
            &candidate,
            &CompileFence {
                registry_revision: evidence.registry_revision,
                grant_revision: evidence.grant_revision,
            },
        )?;
        if compiled.requires_disruption() && !request.allow_disruption {
            return Err(ApplyPlanningError::DisruptionNotAllowed);
        }
        let token = build_token(
            &candidate,
            &compiled,
            &evidence,
            request.allow_disruption,
            expires_at_monotonic_millis,
        )?;
        Ok(ApplyPlan { token, compiled })
    }

    pub fn prepare(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
    ) -> Result<PreparedActivation, ApplyPlanningError> {
        self.revalidate(plan)?;
        let prepared = self.activation.prepare(operation_id, plan)?;
        if prepared.operation_id != *operation_id || prepared.plan_hash != plan.token.plan_hash {
            return Err(ApplyPlanningError::PreconditionChanged(
                PlanPrecondition::PreparedActivation,
            ));
        }
        Ok(prepared)
    }

    /// Rebuilds the plan for a pre-durable-plan commit through the Automation
    /// compiler. This migration path is never used to mask a corrupt plan.
    pub fn compile_recovery(
        &self,
        revision: &WorkspaceRevision,
        candidate_sha256: &str,
        package_lock_sha256: &str,
    ) -> Result<CompiledExecutionPlan, ApplyPlanningError> {
        let evidence = self.evidence.snapshot()?;
        self.compiler
            .compile_recovery(
                RecoveryCompileInput {
                    revision,
                    candidate_sha256,
                    package_lock_sha256,
                },
                &CompileFence {
                    registry_revision: evidence.registry_revision,
                    grant_revision: evidence.grant_revision,
                },
            )
            .map_err(ApplyPlanningError::from)
    }

    pub fn revalidate_for_barrier(
        &self,
        plan: &ApplyPlan,
        prepared: &PreparedActivation,
    ) -> Result<(), ApplyPlanningError> {
        if prepared.plan_hash != plan.token.plan_hash {
            return Err(ApplyPlanningError::PreconditionChanged(
                PlanPrecondition::PreparedActivation,
            ));
        }
        self.revalidate(plan)
    }

    fn revalidate(&self, plan: &ApplyPlan) -> Result<(), ApplyPlanningError> {
        if plan.compiled.semantic_sha256() != plan.token.compiled_plan_sha256
            || calculate_plan_hash(&plan.token)? != plan.token.plan_hash
        {
            return Err(ApplyPlanningError::PreconditionChanged(
                PlanPrecondition::PlanIntegrity,
            ));
        }
        if self.clock.monotonic_millis()? >= plan.token.expires_at_monotonic_millis {
            return Err(ApplyPlanningError::PreconditionChanged(
                PlanPrecondition::Expired,
            ));
        }
        let candidate = self.drafts.get_candidate(&plan.token.draft_id)?;
        validate_candidate(&plan.token, &candidate)?;
        let evidence = self.evidence.snapshot()?;
        validate_evidence(&plan.token, &evidence)
    }
}

fn validate_candidate(
    token: &PlanToken,
    candidate: &WorkspaceCandidate,
) -> Result<(), ApplyPlanningError> {
    let changed = if candidate.version() != token.draft_version {
        Some(PlanPrecondition::DraftVersion)
    } else if candidate.package_lock_sha256() != token.package_lock_sha256 {
        Some(PlanPrecondition::PackageLock)
    } else if candidate.assets_sha256() != token.assets_sha256 {
        Some(PlanPrecondition::Assets)
    } else if candidate.content_sha256() != token.candidate_sha256 {
        Some(PlanPrecondition::CandidateContent)
    } else {
        None
    };
    changed.map_or(Ok(()), |field| {
        Err(ApplyPlanningError::PreconditionChanged(field))
    })
}

fn validate_evidence(
    token: &PlanToken,
    evidence: &PlanningEvidence,
) -> Result<(), ApplyPlanningError> {
    let changed = if evidence.host_boot_id != token.host_boot_id {
        Some(PlanPrecondition::HostBoot)
    } else if evidence.active != token.expected_active {
        Some(PlanPrecondition::ActiveSelection)
    } else if evidence.registry_revision != token.registry_revision {
        Some(PlanPrecondition::RegistryRevision)
    } else if evidence.grant_revision != token.grant_revision {
        Some(PlanPrecondition::GrantRevision)
    } else {
        None
    };
    changed.map_or(Ok(()), |field| {
        Err(ApplyPlanningError::PreconditionChanged(field))
    })
}
