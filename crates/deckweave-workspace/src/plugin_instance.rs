// SPDX-License-Identifier: GPL-3.0-or-later
//! Сохраняемая конфигурация; процесс и физический target имеют отдельные identity.
use deckweave_domain::{
    ids::{ContributionId, CredentialRef, NamespacedKind, PackageId, PluginInstanceId},
    schema::{ExtensionMap, SchemaRef},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstanceConfiguration {
    pub format_version: u32,
    pub instance_id: PluginInstanceId,
    pub package_id: PackageId,
    pub contribution_id: ContributionId,
    pub settings_revision: u64,
    pub settings_schema: SchemaRef,
    pub settings: Value,
    #[serde(default)]
    pub credentials: BTreeMap<String, CredentialRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub service_bindings: BTreeMap<NamespacedKind, crate::plugin_services::ServiceBinding>,
    #[serde(default)]
    pub required_features: BTreeMap<NamespacedKind, u32>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

impl PluginInstanceConfiguration {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.format_version != 1
            || self.settings_revision == 0
            || !deckweave_domain::schema::bounded_json(&self.settings)
            || self.credentials.len() > 64
            || self.required_features.len() > 64
            || self.service_bindings.len() > 64
            || self.required_features.values().any(|v| *v == 0)
            || self
                .credentials
                .keys()
                .any(|key| key.is_empty() || key.len() > 128)
        {
            return Err("invalid configured plugin instance");
        }
        Ok(())
    }
}
