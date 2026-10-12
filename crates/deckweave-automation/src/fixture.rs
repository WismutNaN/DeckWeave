// SPDX-License-Identifier: GPL-3.0-or-later
//! Deliberately restricted compiler used only to prove the Phase 4 apply seam.

use crate::{
    CompileFence, CompiledExecutionPlan, ExecutionPlanKind, PlanCompileError, PlanCompiler,
};
use deckweave_workspace::WorkspaceCandidate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const VIRTUAL_FIXTURE_PACKAGE_ID: &str = "org.deckweave.example.virtual";
const COMPILER_ID: &str = "org.deckweave.compiler.restricted-fixture";
const COMPILER_VERSION: &str = "1";

#[derive(Debug, Default)]
pub struct RestrictedFixtureCompiler;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticInput<'a> {
    compiler_id: &'static str,
    compiler_version: &'static str,
    kind: ExecutionPlanKind,
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
}

impl PlanCompiler for RestrictedFixtureCompiler {
    fn compile(
        &self,
        candidate: &WorkspaceCandidate,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError> {
        let (kind, required_packages) = if candidate
            .document()
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
            && candidate.package_lock().is_empty()
            && candidate.assets().is_empty()
        {
            (ExecutionPlanKind::EmptyFixture, vec![])
        } else {
            let document: VirtualDocument = serde_json::from_value(candidate.document().clone())
                .map_err(|_| PlanCompileError::UnsupportedGraph)?;
            match document.fixture.kind {
                VirtualFixtureKind::VirtualEcho => {}
            }
            if !candidate.assets().is_empty() || candidate.package_lock().len() != 1 {
                return Err(PlanCompileError::UnsupportedGraph);
            }
            let package = &candidate.package_lock()[0];
            if package.package_id.as_str() != VIRTUAL_FIXTURE_PACKAGE_ID {
                return Err(PlanCompileError::UnsupportedGraph);
            }
            (
                ExecutionPlanKind::VirtualEchoFixture,
                vec![package.package_id.clone()],
            )
        };
        let semantic = SemanticInput {
            compiler_id: COMPILER_ID,
            compiler_version: COMPILER_VERSION,
            kind,
            candidate_sha256: candidate.content_sha256(),
            package_lock_sha256: candidate.package_lock_sha256(),
            fence,
        };
        let bytes = serde_json::to_vec(&semantic)
            .map_err(|error| PlanCompileError::InvalidGraph(error.to_string()))?;
        Ok(CompiledExecutionPlan::new(
            COMPILER_ID.into(),
            COMPILER_VERSION.into(),
            kind,
            format!("{:x}", Sha256::digest(bytes)),
            required_packages,
            false,
        ))
    }
}
