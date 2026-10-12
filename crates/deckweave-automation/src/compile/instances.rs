// SPDX-License-Identifier: GPL-3.0-or-later
use super::{CatalogEndpoint, DescriptorCatalog, GraphCompileError};
use crate::GraphNode;
use deckweave_domain::ids::PluginInstanceId;
use deckweave_workspace::PluginInstanceConfiguration;
use std::collections::BTreeMap;

pub(super) fn configure_endpoint(
    node: &GraphNode,
    entry: &CatalogEndpoint,
    catalog: &DescriptorCatalog,
    instances: &BTreeMap<PluginInstanceId, PluginInstanceConfiguration>,
    selected: Option<&PluginInstanceId>,
) -> Result<CatalogEndpoint, GraphCompileError> {
    let mut entry = entry.clone();
    if let Some(id) = selected {
        let invalid = || GraphCompileError::InvalidValue {
            node_id: node.id.clone(),
        };
        let configuration = instances.get(id).ok_or_else(invalid)?;
        configuration.validate().map_err(|_| invalid())?;
        if !configuration.service_bindings.is_empty() {
            return Err(GraphCompileError::InvalidDocument(
                "service bindings require a connected host service broker".into(),
            ));
        }
        if configuration.instance_id != *id
            || configuration.package_id != entry.runtime_requirement.package_id
            || configuration.contribution_id != entry.runtime_requirement.contribution_id
            || !catalog
                .schema(&configuration.settings_schema)
                .is_some_and(|s| s.accepts(&configuration.settings))
        {
            return Err(invalid());
        }
        entry.runtime_requirement.configuration = Some(configuration.clone());
    }
    Ok(entry)
}
