// SPDX-License-Identifier: GPL-3.0-or-later
//! Catalog selection follows the candidate lock, independently of running plugins.
use super::PackageIndex;
use deckweave_automation::{
    CompileFence, CompiledExecutionPlan, GraphLimits, GraphPlanCompiler, PlanCompileError,
    PlanCompiler, RecoveryCompileInput,
};
use deckweave_workspace::{PackageLockEntry, WorkspaceCandidate};
use std::sync::Arc;

pub struct PackageGraphCompiler {
    packages: Arc<PackageIndex>,
    authorization: Option<(
        Arc<dyn deckweave_application::AuthorizationStore>,
        deckweave_domain::ids::PrincipalId,
    )>,
}

impl PackageGraphCompiler {
    pub fn new(packages: Arc<PackageIndex>) -> Self {
        Self {
            packages,
            authorization: None,
        }
    }
    pub fn with_authorization(
        mut self,
        store: Arc<dyn deckweave_application::AuthorizationStore>,
        principal: deckweave_domain::ids::PrincipalId,
    ) -> Self {
        self.authorization = Some((store, principal));
        self
    }
    fn compiler(&self, lock: &[PackageLockEntry]) -> Result<GraphPlanCompiler, PlanCompileError> {
        let catalog = self
            .packages
            .locked_descriptor_catalog(lock)
            .map_err(|e| PlanCompileError::InvalidGraph(e.to_string()))?;
        let mut access = deckweave_automation::CompileAccess::default();
        if let Some((authorization, principal)) = &self.authorization {
            for endpoint in catalog.endpoints() {
                let requirement = &endpoint.runtime_requirement;
                let package = self
                    .packages
                    .resolve_artifact(
                        &requirement.package_id,
                        &requirement.package_version,
                        &requirement.artifact_sha256,
                    )
                    .ok_or_else(|| {
                        PlanCompileError::InvalidGraph("catalog package unavailable".into())
                    })?;
                for scope in &endpoint.port.required_permissions {
                    let permissions: Vec<_> = package
                        .requested_permissions
                        .iter()
                        .filter(|p| {
                            &p.scope == scope
                                && (p.runtime_group_ids.is_empty()
                                    || p.runtime_group_ids.contains(&requirement.runtime_group_id))
                        })
                        .collect();
                    if !permissions.is_empty()
                        && permissions.iter().all(|p| {
                            authorization
                                .authorize(&deckweave_domain::security::PermissionRequest {
                                    principal_id: principal.clone(),
                                    package_id: package.package_id.clone(),
                                    contribution_id: None,
                                    scope: p.scope.clone(),
                                    resource_selector: p.resource_selector.clone(),
                                    access_mode: p.access_mode,
                                    reason: p.reason.clone(),
                                    extensions: Default::default(),
                                })
                                .is_ok()
                        })
                    {
                        access
                            .endpoint_permissions
                            .entry(endpoint.endpoint.clone())
                            .or_default()
                            .insert(scope.clone());
                    }
                }
            }
        }
        GraphPlanCompiler::with_access(catalog, GraphLimits::default(), access)
            .map_err(|e| PlanCompileError::InvalidGraph(e.to_string()))
    }
}

impl PlanCompiler for PackageGraphCompiler {
    fn compile(
        &self,
        candidate: &WorkspaceCandidate,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError> {
        self.compiler(candidate.package_lock())?
            .compile(candidate, fence)
    }
    fn compile_recovery(
        &self,
        input: RecoveryCompileInput<'_>,
        fence: &CompileFence,
    ) -> Result<CompiledExecutionPlan, PlanCompileError> {
        self.compiler(&input.revision.package_lock)?
            .compile_recovery(input, fence)
    }
}
