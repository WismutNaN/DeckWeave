// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    ids::{
        ContributionId, CredentialLeaseId, CredentialRef, GrantId, GrantRevision, NamespacedKind,
        PackageId, PrincipalId,
    },
    schema::ExtensionMap,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAccessMode {
    Observe,
    Read,
    Write,
    Execute,
    Shared,
    Exclusive,
    Provision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub principal_id: PrincipalId,
    pub package_id: PackageId,
    pub contribution_id: Option<ContributionId>,
    pub scope: NamespacedKind,
    pub resource_selector: Value,
    pub access_mode: PermissionAccessMode,
    pub reason: String,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGrant {
    pub grant_id: GrantId,
    pub revision: GrantRevision,
    pub request: PermissionRequest,
    pub expires_at_rfc3339: Option<String>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_instance_id: Option<crate::ids::PluginInstanceId>,
    pub principal_id: PrincipalId,
    pub package_id: PackageId,
    pub credential_ref: CredentialRef,
    pub purpose: NamespacedKind,
    pub deadline_host_ticks: u64,
}

/// Opaque lease: the profile never contains a secret value and consumers do
/// not infer a credential from its display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialLease {
    pub lease_id: CredentialLeaseId,
    pub credential_ref: CredentialRef,
    pub grant_revision: GrantRevision,
    pub expires_at_host_ticks: u64,
}
