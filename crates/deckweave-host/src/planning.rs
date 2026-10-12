// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-owned fencing between immutable candidates, compilation and activation.

use deckweave_automation::{CompileFence, CompiledExecutionPlan, PlanCompileError, PlanCompiler};
use deckweave_domain::ids::{
    DraftId, DraftVersion, GrantRevision, HostBootId, OperationId, RegistryRevision, WorkspaceId,
    WorkspaceRevisionId,
};
use deckweave_workspace::{DraftRepository, DraftRepositoryError, WorkspaceCandidate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSelection {
    pub workspace_id: WorkspaceId,
    pub revision_id: WorkspaceRevisionId,
}

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
        let token = build_token(
            &candidate,
            &compiled,
            &evidence,
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

fn build_token(
    candidate: &WorkspaceCandidate,
    compiled: &CompiledExecutionPlan,
    evidence: &PlanningEvidence,
    expires_at_monotonic_millis: u64,
) -> Result<PlanToken, ApplyPlanningError> {
    let mut token = PlanToken {
        host_boot_id: evidence.host_boot_id.clone(),
        draft_id: candidate.draft_id().clone(),
        workspace_id: candidate.workspace_id().clone(),
        draft_version: candidate.version(),
        candidate_sha256: candidate.content_sha256().into(),
        package_lock_sha256: candidate.package_lock_sha256().into(),
        assets_sha256: candidate.assets_sha256().into(),
        compiled_plan_sha256: compiled.semantic_sha256().into(),
        registry_revision: evidence.registry_revision,
        grant_revision: evidence.grant_revision,
        expected_active: evidence.active.clone(),
        expires_at_monotonic_millis,
        plan_hash: String::new(),
    };
    token.plan_hash = calculate_plan_hash(&token)?;
    Ok(token)
}

fn calculate_plan_hash(token: &PlanToken) -> Result<String, ApplyPlanningError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct HashInput<'a> {
        host_boot_id: &'a HostBootId,
        draft_id: &'a DraftId,
        workspace_id: &'a WorkspaceId,
        draft_version: DraftVersion,
        candidate_sha256: &'a str,
        package_lock_sha256: &'a str,
        assets_sha256: &'a str,
        compiled_plan_sha256: &'a str,
        registry_revision: RegistryRevision,
        grant_revision: GrantRevision,
        expected_active: &'a Option<ActiveSelection>,
        expires_at_monotonic_millis: u64,
    }
    let input = HashInput {
        host_boot_id: &token.host_boot_id,
        draft_id: &token.draft_id,
        workspace_id: &token.workspace_id,
        draft_version: token.draft_version,
        candidate_sha256: &token.candidate_sha256,
        package_lock_sha256: &token.package_lock_sha256,
        assets_sha256: &token.assets_sha256,
        compiled_plan_sha256: &token.compiled_plan_sha256,
        registry_revision: token.registry_revision,
        grant_revision: token.grant_revision,
        expected_active: &token.expected_active,
        expires_at_monotonic_millis: token.expires_at_monotonic_millis,
    };
    let bytes = serde_json::to_vec(&input)
        .map_err(|error| ApplyPlanningError::Serialization(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
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
