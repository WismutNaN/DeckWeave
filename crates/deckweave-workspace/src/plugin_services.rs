// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit instance selection and deterministic dependency closure, without loading code.
use deckweave_domain::{
    ids::{NamespacedKind, PackageId, PluginInstanceId},
    schema::SchemaRef,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceBinding {
    pub provider_instance: PluginInstanceId,
    pub contract: SchemaRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceRequirement {
    pub contract: SchemaRef,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub struct ServiceInstance {
    pub instance_id: PluginInstanceId,
    pub package_id: PackageId,
    pub locked_version: String,
    pub artifact_sha256: String,
    pub provides: Vec<SchemaRef>,
    pub requires: BTreeMap<NamespacedKind, ServiceRequirement>,
    pub bindings: BTreeMap<NamespacedKind, ServiceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ServiceResolutionError {
    #[error("missing explicit provider for {0}")]
    Missing(String),
    #[error("incompatible service contract for {0}")]
    Contract(String),
    #[error("dependency cycle at {0}")]
    Cycle(String),
    #[error("conflicting locked artifact for {0}")]
    Version(String),
    #[error("invalid dependency model or budget exceeded")]
    Invalid,
}

/// Providers precede consumers. Optional roles never select an arbitrary installed provider.
pub fn resolve_services(
    instances: &BTreeMap<PluginInstanceId, ServiceInstance>,
    roots: &[PluginInstanceId],
) -> Result<Vec<PluginInstanceId>, ServiceResolutionError> {
    if instances.len() > 1024 || roots.len() > 1024 {
        return Err(ServiceResolutionError::Invalid);
    }
    let mut visited = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    let mut locks = BTreeMap::new();
    let mut ordered = Vec::new();
    for root in roots {
        visit(
            root,
            instances,
            &mut visited,
            &mut visiting,
            &mut locks,
            &mut ordered,
        )?;
    }
    Ok(ordered)
}

fn visit(
    id: &PluginInstanceId,
    instances: &BTreeMap<PluginInstanceId, ServiceInstance>,
    visited: &mut BTreeSet<PluginInstanceId>,
    visiting: &mut BTreeSet<PluginInstanceId>,
    locks: &mut BTreeMap<PackageId, (String, String)>,
    ordered: &mut Vec<PluginInstanceId>,
) -> Result<(), ServiceResolutionError> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.clone()) {
        return Err(ServiceResolutionError::Cycle(id.to_string()));
    }
    if visiting.len() > 64 {
        return Err(ServiceResolutionError::Invalid);
    }
    let instance = instances
        .get(id)
        .ok_or_else(|| ServiceResolutionError::Missing(id.to_string()))?;
    if &instance.instance_id != id
        || instance.requires.len() > 64
        || instance.provides.len() > 64
        || instance.artifact_sha256.len() != 64
        || !instance
            .artifact_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || instance.locked_version.is_empty()
        || instance.locked_version.len() > 128
        || instance
            .bindings
            .keys()
            .any(|role| !instance.requires.contains_key(role))
    {
        return Err(ServiceResolutionError::Invalid);
    }
    let lock = (
        instance.locked_version.clone(),
        instance.artifact_sha256.clone(),
    );
    if locks
        .get(&instance.package_id)
        .is_some_and(|known| known != &lock)
    {
        return Err(ServiceResolutionError::Version(
            instance.package_id.to_string(),
        ));
    }
    locks.insert(instance.package_id.clone(), lock);
    for (role, requirement) in &instance.requires {
        let Some(binding) = instance.bindings.get(role) else {
            if requirement.optional {
                continue;
            }
            return Err(ServiceResolutionError::Missing(role.to_string()));
        };
        let provider = instances.get(&binding.provider_instance).ok_or_else(|| {
            ServiceResolutionError::Missing(binding.provider_instance.to_string())
        })?;
        if binding.contract != requirement.contract
            || !provider.provides.contains(&binding.contract)
        {
            return Err(ServiceResolutionError::Contract(role.to_string()));
        }
        visit(
            &binding.provider_instance,
            instances,
            visited,
            visiting,
            locks,
            ordered,
        )?;
    }
    visiting.remove(id);
    visited.insert(id.clone());
    ordered.push(id.clone());
    Ok(())
}
