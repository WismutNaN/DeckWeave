// SPDX-License-Identifier: GPL-3.0-or-later
//! Deliberately restricted compiler used only to prove the Phase 4 apply seam.

use crate::{
    CompileFence, CompiledExecutionPlan, PlanCompileError, PlanCompiler, RecoveryCompileInput,
    RuntimeRequirement,
};
use deckweave_domain::ids::{ContributionId, RuntimeGroupId};
use deckweave_workspace::{AssetReference, PackageLockEntry, WorkspaceCandidate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const VIRTUAL_FIXTURE_PACKAGE_ID: &str = "org.deckweave.example.virtual";
pub const VIRTUAL_FIXTURE_RUNTIME_GROUP_ID: &str = "control";
pub const VIRTUAL_FIXTURE_CONTRIBUTION_ID: &str = "virtual-device";
const COMPILER_ID: &str = "org.deckweave.compiler.restricted-fixture";
const COMPILER_VERSION: &str = "1";

#[derive(Debug, Default)]
pub struct RestrictedFixtureCompiler;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticInput<'a> {
    compiler_id: &'static str,
    compiler_version: &'static str,
    runtime_requirements: &'a [RuntimeRequirement],
    candidate_sha256: &'a str,
    package_lock_sha256: &'a str,
    fence: &'a CompileFence,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VirtualDocument {
    fixture: VirtualFixture,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VirtualFixture {
    kind: VirtualFixtureKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VirtualFixtureKind {
    VirtualEcho,
    VirtualDisruptiveEcho,
}

impl PlanCompiler for RestrictedFixtureCompiler {
    fn compile(
        &self,
        candidate: &WorkspaceCandidate,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError> {
        compile_document(
            candidate.document(),
            candidate.package_lock(),
            candidate.assets(),
            candidate.content_sha256(),
            candidate.package_lock_sha256(),
            fence,
        )
    }

    fn compile_recovery(
        &self,
        input: RecoveryCompileInput<'_>,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError> {
        compile_document(
            &input.revision.document,
            &input.revision.package_lock,
            &input.revision.assets,
            input.candidate_sha256,
            input.package_lock_sha256,
            fence,
        )
    }
}

fn compile_document(
    document: &serde_json::Value,
    package_lock: &[PackageLockEntry],
    assets: &[AssetReference],
    candidate_sha256: &str,
    package_lock_sha256: &str,
    fence: &CompileFence,
) -> Result<CompiledExecutionPlan, PlanCompileError> {
    let (runtime_requirements, requires_disruption) =
        if document.as_object().is_some_and(serde_json::Map::is_empty)
            && package_lock.is_empty()
            && assets.is_empty()
        {
            (vec![], false)
        } else {
            let document: VirtualDocument = serde_json::from_value(document.clone())
                .map_err(|_| PlanCompileError::UnsupportedGraph)?;
            let requires_disruption = matches!(
                document.fixture.kind,
                VirtualFixtureKind::VirtualDisruptiveEcho
            );
            if !assets.is_empty() || package_lock.len() != 1 {
                return Err(PlanCompileError::UnsupportedGraph);
            }
            let package = &package_lock[0];
            if package.package_id.as_str() != VIRTUAL_FIXTURE_PACKAGE_ID {
                return Err(PlanCompileError::UnsupportedGraph);
            }
            (
                vec![RuntimeRequirement {
                    package_id: package.package_id.clone(),
                    package_version: package.version.clone(),
                    artifact_sha256: package.artifact_sha256.clone(),
                    runtime_group_id: RuntimeGroupId::try_new(VIRTUAL_FIXTURE_RUNTIME_GROUP_ID)
                        .map_err(|error| PlanCompileError::InvalidGraph(error.to_string()))?,
                    contribution_id: ContributionId::try_new(VIRTUAL_FIXTURE_CONTRIBUTION_ID)
                        .map_err(|error| PlanCompileError::InvalidGraph(error.to_string()))?,
                }],
                requires_disruption,
            )
        };
    let semantic = SemanticInput {
        compiler_id: COMPILER_ID,
        compiler_version: COMPILER_VERSION,
        runtime_requirements: &runtime_requirements,
        candidate_sha256,
        package_lock_sha256,
        fence,
    };
    let bytes = serde_json::to_vec(&semantic)
        .map_err(|error| PlanCompileError::InvalidGraph(error.to_string()))?;
    Ok(CompiledExecutionPlan::new(
        COMPILER_ID.into(),
        COMPILER_VERSION.into(),
        format!("{:x}", Sha256::digest(bytes)),
        runtime_requirements,
        requires_disruption,
    ))
}
