// SPDX-License-Identifier: GPL-3.0-or-later
use super::GraphPortId;
use deckweave_domain::ids::GraphNodeId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionArguments {
    pub trigger_port: GraphPortId,
    pub fields: BTreeMap<String, ValueBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ValueBinding {
    Constant {
        value: Value,
    },
    Trigger {
        pointer: String,
    },
    State {
        node_id: GraphNodeId,
        pointer: String,
    },
    RunResult {
        node_id: GraphNodeId,
        pointer: String,
    },
}

impl ActionArguments {
    pub fn assemble(
        &self,
        trigger: &Value,
        states: &BTreeMap<GraphNodeId, Value>,
        results: &BTreeMap<GraphNodeId, Value>,
    ) -> Option<Value> {
        let mut fields = Map::new();
        for (name, binding) in &self.fields {
            let value = match binding {
                ValueBinding::Constant { value } => value,
                ValueBinding::Trigger { pointer } => trigger.pointer(pointer)?,
                ValueBinding::State { node_id, pointer } => {
                    states.get(node_id)?.pointer(pointer)?
                }
                ValueBinding::RunResult { node_id, pointer } => {
                    results.get(node_id)?.pointer(pointer)?
                }
            };
            fields.insert(name.clone(), value.clone());
        }
        Some(Value::Object(fields))
    }
}
