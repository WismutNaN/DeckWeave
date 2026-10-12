// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{InteractionSessionId, InvocationId, ProviderId};
use deckweave_domain::{
    ids::{ActivationEpoch, SessionGeneration},
    schema::{PayloadSchema, SchemaRef},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionViewKind {
    SearchList,
    Form,
    Confirm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionRequest {
    pub session_id: InteractionSessionId,
    pub owner_invocation_id: InvocationId,
    pub provider_id: ProviderId,
    pub activation_epoch: ActivationEpoch,
    pub provider_generation: SessionGeneration,
    pub view: InteractionViewKind,
    pub initial_query: serde_json::Value,
    pub result_schema: SchemaRef,
    pub result_shape: PayloadSchema,
    pub deadline_host_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionItem {
    pub item_id: String,
    pub label: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionPage {
    pub session_id: InteractionSessionId,
    pub fence: QueryFence,
    pub items: Vec<InteractionItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelReason {
    UserClosed,
    Escape,
    Apply,
    Revoked,
    ProviderLost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum InteractionResult {
    Selected(serde_json::Value),
    Cancelled(CancelReason),
    Failed(String),
    TimedOut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionSession {
    pub generation: u64,
    pub request: InteractionRequest,
    pub page: Option<InteractionPage>,
    pub result: Option<InteractionResult>,
    pub query_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFence {
    pub session_generation: u64,
    pub activation_epoch: ActivationEpoch,
    pub provider_generation: SessionGeneration,
    pub query_generation: u64,
}
