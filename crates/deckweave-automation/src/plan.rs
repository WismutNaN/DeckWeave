// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{
    ContributionId, GrantRevision, PackageId, RegistryRevision, RuntimeGroupId,
};
use deckweave_workspace::{WorkspaceCandidate, WorkspaceRevision};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const COMPILED_PLAN_EXTENSION_ID: &str = "org.deckweave.automation.compiled-plan.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileFence {
    pub registry_revision: RegistryRevision,
    pub grant_revision: GrantRevision,
}

/// Immutable input used only to migrate commits created before compiled plans
/// became part of the apply record. The caller supplies the original hashes
/// because a committed revision intentionally omits draft-only identity.
#[derive(Debug, Clone, Copy)]
pub struct RecoveryCompileInput<'a> {
    pub revision: &'a WorkspaceRevision,
    pub candidate_sha256: &'a str,
    pub package_lock_sha256: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRequirement {
    pub package_id: PackageId,
    pub package_version: String,
    pub artifact_sha256: String,
    pub runtime_group_id: RuntimeGroupId,
    pub contribution_id: ContributionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledExecutionPlan {
    compiler_id: String,
    compiler_version: String,
    semantic_sha256: String,
    runtime_requirements: Vec<RuntimeRequirement>,
    requires_disruption: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableExecutionPlan {
    document_sha256: String,
    plan: CompiledExecutionPlan,
}

impl CompiledExecutionPlan {
    pub(crate) fn new(
        compiler_id: String,
        compiler_version: String,
        semantic_sha256: String,
        runtime_requirements: Vec<RuntimeRequirement>,
        requires_disruption: bool,
    ) -> Self {
        Self {
            compiler_id,
            compiler_version,
            semantic_sha256,
            runtime_requirements,
            requires_disruption,
        }
    }

    #[must_use]
    pub fn compiler_id(&self) -> &str {
        &self.compiler_id
    }

    #[must_use]
    pub fn compiler_version(&self) -> &str {
        &self.compiler_version
    }

    #[must_use]
    pub fn semantic_sha256(&self) -> &str {
        &self.semantic_sha256
    }

    #[must_use]
    pub fn runtime_requirements(&self) -> &[RuntimeRequirement] {
        &self.runtime_requirements
    }

    #[must_use]
    pub fn requires_disruption(&self) -> bool {
        self.requires_disruption
    }

    pub fn seal(&self) -> Result<DurableExecutionPlan, PlanCompileError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| PlanCompileError::InvalidGraph(error.to_string()))?;
        Ok(DurableExecutionPlan {
            document_sha256: format!("{:x}", Sha256::digest(bytes)),
            plan: self.clone(),
        })
    }
}

impl DurableExecutionPlan {
    pub fn verify(self) -> Result<CompiledExecutionPlan, PlanCompileError> {
        let bytes = serde_json::to_vec(&self.plan)
            .map_err(|error| PlanCompileError::InvalidGraph(error.to_string()))?;
        if format!("{:x}", Sha256::digest(bytes)) != self.document_sha256 {
            return Err(PlanCompileError::InvalidGraph(
                "durable compiled plan checksum mismatch".into(),
            ));
        }
        Ok(self.plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanCompileError {
    #[error("workspace graph is not supported by the current compiler")]
    UnsupportedGraph,
    #[error("workspace graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("plan compiler is unavailable: {0}")]
    Unavailable(String),
}

pub trait PlanCompiler: Send + Sync {
    fn compile(
        &self,
        candidate: &WorkspaceCandidate,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError>;

    fn compile_recovery(
        &self,
        input: RecoveryCompileInput<'_>,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError>;
}
