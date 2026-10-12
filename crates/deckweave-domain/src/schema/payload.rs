// SPDX-License-Identifier: GPL-3.0-or-later
//! Ограниченное структурное описание control payload. Не полный JSON Schema.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PayloadSchema {
    Null,
    Boolean,
    Number {
        minimum: f64,
        maximum: f64,
    },
    String {
        maximum_bytes: usize,
        #[serde(default)]
        choices: Vec<String>,
    },
    Array {
        items: Box<PayloadSchema>,
        maximum_items: usize,
    },
    Record {
        fields: BTreeMap<String, PayloadField>,
    },
    Nullable {
        inner: Box<PayloadSchema>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PayloadField {
    pub schema: PayloadSchema,
    #[serde(default)]
    pub optional: bool,
}

impl PayloadSchema {
    pub fn validate_definition(&self) -> Result<(), &'static str> {
        let mut remaining = 4096;
        self.definition(0, &mut remaining)
    }

    fn definition(&self, depth: usize, remaining: &mut usize) -> Result<(), &'static str> {
        if depth > 32 || *remaining == 0 {
            return Err("schema complexity exceeded");
        }
        *remaining -= 1;
        match self {
            Self::Number { minimum, maximum }
                if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum =>
            {
                Err("invalid numeric bounds")
            }
            Self::String {
                maximum_bytes,
                choices,
            } => {
                if *maximum_bytes > 1024 * 1024
                    || choices.len() > 1024
                    || choices.iter().any(|s| s.len() > *maximum_bytes)
                {
                    Err("invalid string bounds")
                } else {
                    Ok(())
                }
            }
            Self::Array {
                items,
                maximum_items,
            } => {
                if *maximum_items > 65536 {
                    return Err("invalid array bounds");
                }
                items.definition(depth + 1, remaining)
            }
            Self::Record { fields } => {
                for (name, field) in fields {
                    if name.is_empty() || name.len() > 128 {
                        return Err("invalid field name");
                    }
                    field.schema.definition(depth + 1, remaining)?;
                }
                Ok(())
            }
            Self::Nullable { inner } => inner.definition(depth + 1, remaining),
            _ => Ok(()),
        }
    }

    #[must_use]
    pub fn accepts(&self, value: &Value) -> bool {
        if !bounded_json(value) {
            return false;
        }
        self.accepts_inner(value, 0)
    }

    fn accepts_inner(&self, value: &Value, depth: usize) -> bool {
        if depth > 32 {
            return false;
        }
        match self {
            Self::Null => value.is_null(),
            Self::Boolean => value.is_boolean(),
            Self::Number { minimum, maximum } => value
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= *minimum && n <= *maximum),
            Self::String {
                maximum_bytes,
                choices,
            } => value.as_str().is_some_and(|s| {
                s.len() <= *maximum_bytes
                    && (choices.is_empty() || choices.iter().any(|choice| choice == s))
            }),
            Self::Array {
                items,
                maximum_items,
            } => value.as_array().is_some_and(|values| {
                values.len() <= *maximum_items
                    && values.iter().all(|v| items.accepts_inner(v, depth + 1))
            }),
            Self::Record { fields } => value.as_object().is_some_and(|object| {
                object.keys().all(|key| fields.contains_key(key))
                    && fields.iter().all(|(name, field)| match object.get(name) {
                        None => field.optional,
                        Some(value) => field.schema.accepts_inner(value, depth + 1),
                    })
            }),
            Self::Nullable { inner } => value.is_null() || inner.accepts_inner(value, depth + 1),
        }
    }
}

/// Общий предел применяется также к явно opaque JSON extension payload.
#[must_use]
pub fn bounded_json(value: &Value) -> bool {
    fn visit(value: &Value, depth: usize, nodes: &mut usize, bytes: &mut usize) -> bool {
        if depth > 32 || *nodes == 0 {
            return false;
        }
        *nodes -= 1;
        let size = match value {
            Value::String(s) => s.len(),
            Value::Object(o) => o.keys().map(String::len).sum(),
            _ => 0,
        };
        if size > *bytes {
            return false;
        }
        *bytes -= size;
        match value {
            Value::Array(a) => a.iter().all(|v| visit(v, depth + 1, nodes, bytes)),
            Value::Object(o) => o.values().all(|v| visit(v, depth + 1, nodes, bytes)),
            _ => true,
        }
    }
    visit(value, 0, &mut 65536, &mut (1024 * 1024))
}
