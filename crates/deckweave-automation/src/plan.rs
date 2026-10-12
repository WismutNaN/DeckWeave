// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{GrantRevision, PackageId, RegistryRevision};
use deckweave_workspace::WorkspaceCandidate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileFence {
    pub registry_revision: RegistryRevision,
    pub grant_revision: GrantRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlanKind {
    EmptyFixture,
    VirtualEchoFixture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledExecutionPlan {
    compiler_id: String,
    compiler_version: String,
    kind: ExecutionPlanKind,
    semantic_sha256: String,
    required_packages: Vec<PackageId>,
    requires_disruption: bool,
}

impl CompiledExecutionPlan {
    pub(crate) fn new(
        compiler_id: String,
        compiler_version: String,
        kind: ExecutionPlanKind,
        semantic_sha256: String,
        required_packages: Vec<PackageId>,
        requires_disruption: bool,
    ) -> Self {
        Self {
            compiler_id,
            compiler_version,
            kind,
            semantic_sha256,
            required_packages,
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
    pub fn kind(&self) -> ExecutionPlanKind {
        self.kind
    }

    #[must_use]
    pub fn semantic_sha256(&self) -> &str {
        &self.semantic_sha256
    }

    #[must_use]
    pub fn required_packages(&self) -> &[PackageId] {
        &self.required_packages
    }

    #[must_use]
    pub fn requires_disruption(&self) -> bool {
        self.requires_disruption
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
}
