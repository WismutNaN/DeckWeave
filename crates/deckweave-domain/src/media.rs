// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{ids::NamespacedKind, schema::ExtensionMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamAvailability {
    Always,
    Dynamic,
    Requestable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormatConstraint {
    pub media_type: String,
    #[serde(default)]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamBufferBounds {
    pub minimum_bytes: u64,
    pub maximum_bytes: u64,
    pub maximum_in_flight: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamTemplate {
    pub template_id: NamespacedKind,
    pub direction: StreamDirection,
    pub availability: StreamAvailability,
    #[serde(default)]
    pub formats: Vec<MediaFormatConstraint>,
    pub buffer_bounds: StreamBufferBounds,
    pub maximum_latency_ms: u32,
    pub clock_domain_required: bool,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StreamTemplateError {
    #[error("stream template должен объявить хотя бы один format")]
    MissingFormat,
    #[error("buffer/latency bounds должны быть ненулевыми и упорядоченными")]
    InvalidBounds,
}

impl StreamTemplate {
    pub fn validate(&self) -> Result<(), StreamTemplateError> {
        if self.formats.is_empty() {
            return Err(StreamTemplateError::MissingFormat);
        }
        if self.buffer_bounds.minimum_bytes == 0
            || self.buffer_bounds.minimum_bytes > self.buffer_bounds.maximum_bytes
            || self.buffer_bounds.maximum_in_flight == 0
            || self.maximum_latency_ms == 0
        {
            return Err(StreamTemplateError::InvalidBounds);
        }
        Ok(())
    }
}
