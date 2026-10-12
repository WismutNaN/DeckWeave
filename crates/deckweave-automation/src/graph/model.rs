// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_domain::{
    capability::NumericRange,
    endpoint::StableEndpointRef,
    ids::{GraphNodeId, NamespacedKind, OperationId},
    schema::{ExtensionMap, SchemaRef},
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphFormatVersion {
    pub major: u32,
    pub minor: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDocument {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugin_instances: BTreeMap<
        deckweave_domain::ids::PluginInstanceId,
        deckweave_workspace::PluginInstanceConfiguration,
    >,
    pub schema_version: GraphFormatVersion,
    pub graph: GraphDefinition,
    #[serde(default)]
    pub layout: GraphLayout,
    #[serde(default)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDefinition {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub node_instances: BTreeMap<GraphNodeId, deckweave_domain::ids::PluginInstanceId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub action_arguments: BTreeMap<GraphNodeId, super::ActionArguments>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    #[serde(default)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNode {
    pub id: GraphNodeId,
    pub kind: GraphNodeKind,
    #[serde(default)]
    pub inputs: Vec<GraphPort>,
    #[serde(default)]
    pub outputs: Vec<GraphPort>,
    #[serde(default)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphNodeKind {
    Source {
        endpoint: StableEndpointRef,
        #[serde(default)]
        availability: EndpointAvailabilityPolicy,
    },
    Action {
        endpoint: StableEndpointRef,
        operation_id: OperationId,
        #[serde(default)]
        availability: EndpointAvailabilityPolicy,
    },
    State {
        state_key: NamespacedKind,
        initial_value: Option<Value>,
    },
    Condition {
        operator: ConditionOperator,
        operand: Option<Value>,
    },
    Feedback {
        endpoint: StableEndpointRef,
        #[serde(default)]
        availability: EndpointAvailabilityPolicy,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointAvailabilityPolicy {
    Required,
    #[default]
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    IsTrue,
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphPort {
    pub id: GraphPortId,
    pub kind: GraphPortKind,
    pub role: GraphPortRole,
    pub schema: SchemaRef,
    pub unit: Option<String>,
    pub range: Option<NumericRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphPortKind {
    Flow,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphPortRole {
    Trigger,
    Input,
    Result,
    Error,
    Value,
    True,
    False,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphPortRef {
    pub node_id: GraphNodeId,
    pub port_id: GraphPortId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEdge {
    pub id: GraphEdgeId,
    pub kind: GraphEdgeKind,
    pub source: GraphPortRef,
    pub target: GraphPortRef,
    #[serde(default)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeKind {
    Flow,
    Data,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphLayout {
    #[serde(default)]
    pub nodes: BTreeMap<GraphNodeId, GraphNodePlacement>,
    #[serde(default)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNodePlacement {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub collapsed: bool,
}

macro_rules! graph_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, GraphIdentityError> {
                let value = value.into();
                validate_graph_id(stringify!($name), &value)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphIdentityError {
    #[error("{kind} must contain 1..=128 safe ASCII characters")]
    Invalid { kind: &'static str },
}

fn validate_graph_id(kind: &'static str, value: &str) -> Result<(), GraphIdentityError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-:/".contains(&byte))
    {
        return Err(GraphIdentityError::Invalid { kind });
    }
    Ok(())
}

graph_id!(GraphEdgeId);
graph_id!(GraphPortId);
