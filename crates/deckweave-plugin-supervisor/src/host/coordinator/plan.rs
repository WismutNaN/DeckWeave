// SPDX-License-Identifier: GPL-3.0-or-later
//! Validation and binding of a compiled plan to prepared runtime instances.

use super::{HostRuntimeError, RuntimeInstance};
use deckweave_automation::{
    BoundExecutionGraph, CompiledExecutionPlan, RuntimeBinder, RuntimeRequirement,
};
use std::collections::BTreeSet;

pub(super) fn validate_distinct_runtime_groups(
    requirements: &[RuntimeRequirement],
) -> Result<(), HostRuntimeError> {
    let mut groups = BTreeSet::new();
    for requirement in requirements {
        let group = (
            requirement.package_id.clone(),
            requirement.runtime_group_id.clone(),
        );
        if !groups.insert(group) {
            return Err(HostRuntimeError::Package(format!(
                "runtime group {} has multiple compiled contributions; grouped activation is not supported yet",
                requirement.runtime_group_id
            )));
        }
    }
    Ok(())
}

pub(super) fn bind_instances(
    plan: &CompiledExecutionPlan,
    instances: &[RuntimeInstance],
) -> Result<BoundExecutionGraph, HostRuntimeError> {
    let graph = plan
        .executable_graph()
        .ok_or_else(|| HostRuntimeError::Protocol("compiled graph is missing".into()))?;
    let mut bindings = Vec::new();
    for instance in instances {
        bindings.extend(instance.bindings(&graph.endpoint_contracts)?);
    }
    RuntimeBinder
        .bind_plan(plan, &bindings)
        .map_err(|error| HostRuntimeError::Protocol(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_domain::ids::{ContributionId, PackageId, RuntimeGroupId};

    fn requirement(
        package: &str,
        group: &str,
        contribution: &str,
    ) -> Result<RuntimeRequirement, Box<dyn std::error::Error>> {
        Ok(RuntimeRequirement {
            package_id: PackageId::try_new(package)?,
            package_version: "1.0.0".into(),
            artifact_sha256: "a".repeat(64),
            runtime_group_id: RuntimeGroupId::try_new(group)?,
            contribution_id: ContributionId::try_new(contribution)?,
        })
    }

    #[test]
    fn distinct_runtime_groups_are_accepted() -> Result<(), Box<dyn std::error::Error>> {
        validate_distinct_runtime_groups(&[
            requirement("org.deckweave.first", "control", "device")?,
            requirement("org.deckweave.second", "control", "service")?,
        ])?;
        Ok(())
    }

    #[test]
    fn two_contributions_cannot_start_duplicate_processes_for_one_group()
    -> Result<(), Box<dyn std::error::Error>> {
        let requirements = [
            requirement("org.deckweave.first", "control", "device")?,
            requirement("org.deckweave.first", "control", "service")?,
        ];
        assert!(matches!(
            validate_distinct_runtime_groups(&requirements),
            Err(HostRuntimeError::Package(_))
        ));
        Ok(())
    }
}
