// SPDX-License-Identifier: GPL-3.0-or-later
//! Construction and integrity hashing for boot-scoped apply plan tokens.

use super::*;
use sha2::{Digest, Sha256};

pub(super) fn build_token(
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

pub(super) fn calculate_plan_hash(token: &PlanToken) -> Result<String, ApplyPlanningError> {
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
