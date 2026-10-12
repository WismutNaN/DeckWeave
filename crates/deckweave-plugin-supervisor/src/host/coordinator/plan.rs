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
    let mut groups = std::collections::BTreeMap::new();
    let mut activations = BTreeSet::new();
    let mut versions = std::collections::BTreeMap::new();
    for requirement in requirements {
        if versions
            .insert(&requirement.package_id, &requirement.package_version)
            .is_some_and(|previous| previous != &requirement.package_version)
        {
            return Err(HostRuntimeError::Package(
                "multiple active versions of one package".into(),
            ));
        }
        if let Some(config) = &requirement.configuration {
            config
                .validate()
                .map_err(|e| HostRuntimeError::Package(e.into()))?;
            if config.package_id != requirement.package_id
                || config.contribution_id != requirement.contribution_id
            {
                return Err(HostRuntimeError::Package(
                    "configured instance identity mismatch".into(),
                ));
            }
        }
        let group = (
            requirement.package_id.clone(),
            requirement.runtime_group_id.clone(),
        );
        let artifact = (&requirement.package_version, &requirement.artifact_sha256);
        if groups
            .insert(group.clone(), artifact)
            .is_some_and(|previous| previous != artifact)
        {
            return Err(HostRuntimeError::Package(format!(
                "runtime group {} has conflicting locked artifacts",
                requirement.runtime_group_id
            )));
        }
        if !activations.insert((
            group,
            &requirement.contribution_id,
            requirement.configuration.as_ref().map(|c| &c.instance_id),
        )) {
            return Err(HostRuntimeError::Package(
                "duplicate configured activation".into(),
            ));
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
            configuration: None,
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
    fn two_contributions_share_one_group_but_duplicate_activation_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let requirements = [
            requirement("org.deckweave.first", "control", "device")?,
            requirement("org.deckweave.first", "control", "service")?,
        ];
        validate_distinct_runtime_groups(&requirements)?;
        assert!(matches!(
            validate_distinct_runtime_groups(&[requirements[0].clone(), requirements[0].clone()]),
            Err(HostRuntimeError::Package(_))
        ));
        Ok(())
    }
}
