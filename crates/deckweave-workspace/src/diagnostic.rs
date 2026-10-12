// SPDX-License-Identifier: GPL-3.0-or-later
//! Стабильная проекция ошибки draft; не содержит значения полей или секреты.
use deckweave_domain::ids::GraphNodeId;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftDiagnostic {
    pub code: String,
    pub node_id: Option<GraphNodeId>,
    pub field: Option<String>,
}
