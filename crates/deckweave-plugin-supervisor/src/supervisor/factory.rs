// SPDX-License-Identifier: GPL-3.0-or-later
//! Validated initial state for one runtime controller generation.

use super::*;

pub(super) fn runtime_controller(
    package: Arc<IndexedPackage>,
    runtime_group_id: RuntimeGroupId,
    policy: RestartPolicy,
    previous_generation: u64,
) -> Result<RuntimeController, SupervisorError> {
    validate_policy(&policy)?;
    let artifact = package
        .artifact_for_current_platform(&runtime_group_id)
        .map_err(|error| SupervisorError::Artifact(error.to_string()))?;
    Ok(RuntimeController {
        key: RuntimeKey {
            package_id: package.package_id.clone(),
            artifact_sha256: artifact.sha256.clone(),
            runtime_group_id,
        },
        package,
        policy,
        status: RuntimeStatus::Indexed,
        leases: Vec::new(),
        grants: Vec::new(),
        session: None,
        one_time_token: None,
        next_lease: 0,
        next_generation: previous_generation,
        transition_deadline: None,
        retry_at: None,
        crashes: VecDeque::new(),
        registered: false,
        restart_blocked: false,
        last_error: None,
    })
}
