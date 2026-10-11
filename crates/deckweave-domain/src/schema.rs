// SPDX-License-Identifier: GPL-3.0-or-later
use crate::ids::NamespacedKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

pub type ExtensionMap = BTreeMap<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaRef {
    pub id: NamespacedKind,
    pub version: SchemaVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaVersionRange {
    pub minimum: SchemaVersion,
    pub maximum: SchemaVersion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionDescriptor {
    pub kind: NamespacedKind,
    pub schema: SchemaRef,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchemaError {
    #[error("minimum schema version выше maximum")]
    InvalidRange,
    #[error("несовместимая major schema version")]
    MajorMismatch,
}

impl SchemaVersionRange {
    pub fn validate(&self) -> Result<(), SchemaError> {
        if self.minimum > self.maximum {
            return Err(SchemaError::InvalidRange);
        }
        if self.minimum.major != self.maximum.major {
            return Err(SchemaError::MajorMismatch);
        }
        Ok(())
    }

    #[must_use]
    pub fn contains(&self, version: SchemaVersion) -> bool {
        self.minimum <= version && version <= self.maximum
    }
}
