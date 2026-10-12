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
    #[error(
        "потоки требуют явно выбранный adapter формата или имеют несовместимые bounds/direction"
    )]
    Incompatible,
    #[error("stream template должен объявить хотя бы один format")]
    MissingFormat,
    #[error("buffer/latency bounds должны быть ненулевыми и упорядоченными")]
    InvalidBounds,
}

impl StreamTemplate {
    /// Согласование консервативное: неизвестные свойства не игнорируются,
    /// resampling/color conversion/clock sync требуют явного adapter.
    pub fn negotiate_with(&self, consumer: &Self) -> Result<NegotiatedStream, StreamTemplateError> {
        self.validate()?;
        consumer.validate()?;
        if self.direction != StreamDirection::Output || consumer.direction != StreamDirection::Input
        {
            return Err(StreamTemplateError::Incompatible);
        }
        let format = consumer
            .formats
            .iter()
            .find(|wanted| self.formats.contains(wanted))
            .cloned()
            .ok_or(StreamTemplateError::Incompatible)?;
        let minimum = self
            .buffer_bounds
            .minimum_bytes
            .max(consumer.buffer_bounds.minimum_bytes);
        let maximum = self
            .buffer_bounds
            .maximum_bytes
            .min(consumer.buffer_bounds.maximum_bytes);
        if minimum > maximum {
            return Err(StreamTemplateError::Incompatible);
        }
        Ok(NegotiatedStream {
            format,
            buffer_bounds: StreamBufferBounds {
                minimum_bytes: minimum,
                maximum_bytes: maximum,
                maximum_in_flight: self
                    .buffer_bounds
                    .maximum_in_flight
                    .min(consumer.buffer_bounds.maximum_in_flight),
            },
            maximum_latency_ms: self.maximum_latency_ms.min(consumer.maximum_latency_ms),
            clock_domain_required: self.clock_domain_required || consumer.clock_domain_required,
        })
    }
    pub fn validate(&self) -> Result<(), StreamTemplateError> {
        if self.formats.is_empty() {
            return Err(StreamTemplateError::MissingFormat);
        }
        if self.formats.len() > 64
            || self.formats.iter().any(|format| {
                format.media_type.is_empty()
                    || format.media_type.len() > 128
                    || format.properties.len() > 64
                    || format
                        .properties
                        .values()
                        .any(|v| !crate::schema::bounded_json(v))
            })
        {
            return Err(StreamTemplateError::InvalidBounds);
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NegotiatedStream {
    pub format: MediaFormatConstraint,
    pub buffer_bounds: StreamBufferBounds,
    pub maximum_latency_ms: u32,
    pub clock_domain_required: bool,
}
