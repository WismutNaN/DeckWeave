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
    #[serde(default)]
    pub fields: BTreeMap<String, ValueBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<ValueBinding>,
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
    /// All dependencies, including a binding of the entire operation input.
    pub fn bindings(&self) -> impl Iterator<Item = &ValueBinding> {
        self.fields.values().chain(self.value.iter())
    }

    pub fn assemble(
        &self,
        trigger: &Value,
        states: &BTreeMap<GraphNodeId, Value>,
        results: &BTreeMap<GraphNodeId, Value>,
    ) -> Option<Value> {
        if let Some(binding) = &self.value {
            return self
                .fields
                .is_empty()
                .then(|| binding.resolve(trigger, states, results).cloned())
                .flatten();
        }
        let mut fields = Map::new();
        for (name, binding) in &self.fields {
            let value = binding.resolve(trigger, states, results)?;
            fields.insert(name.clone(), value.clone());
        }
        Some(Value::Object(fields))
    }
}

impl ValueBinding {
    fn resolve<'a>(
        &'a self,
        trigger: &'a Value,
        states: &'a BTreeMap<GraphNodeId, Value>,
        results: &'a BTreeMap<GraphNodeId, Value>,
    ) -> Option<&'a Value> {
        match self {
            Self::Constant { value } => Some(value),
            Self::Trigger { pointer } => trigger.pointer(pointer),
            Self::State { node_id, pointer } => states.get(node_id)?.pointer(pointer),
            Self::RunResult { node_id, pointer } => results.get(node_id)?.pointer(pointer),
        }
    }
}
