// SPDX-License-Identifier: GPL-3.0-or-later
//! Persisted direct endpoint binding vocabulary for workspace documents.

use deckweave_domain::{endpoint::StableEndpointRef, ids::BindingId, schema::ExtensionMap};
use serde::{Deserialize, Serialize};

/// A persisted binding contains stable identities only. Runtime placement is
/// resolved and fenced while compiling an activation plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointBinding {
    pub binding_id: BindingId,
    pub source: StableEndpointRef,
    pub target: StableEndpointRef,
    pub enabled: bool,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}
