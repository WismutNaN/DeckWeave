// SPDX-License-Identifier: GPL-3.0-or-later
//! Carrier-independent remote capability descriptors. Transport sessions and
//! media buffer ownership deliberately remain outside the Phase 2 domain.

use crate::{
    ids::{NamespacedKind, NodeId},
    schema::{ExtensionMap, SchemaRef},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteChannelKind {
    Control,
    Media,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteChannelDescriptor {
    pub kind: RemoteChannelKind,
    /// Namespaced carrier capability, for example `org.deckweave.transport.quic`.
    pub carrier: NamespacedKind,
    #[serde(default)]
    pub schemas: Vec<SchemaRef>,
    pub maximum_message_bytes: u64,
    pub clock_uncertainty_ns: Option<u64>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteNodeBoundary {
    pub node_id: NodeId,
    #[serde(default)]
    pub channels: Vec<RemoteChannelDescriptor>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RemoteBoundaryError {
    #[error("remote channel maximum_message_bytes должен быть больше нуля")]
    ZeroMessageLimit,
    #[error("remote boundary содержит повторяющийся channel kind")]
    DuplicateChannel,
}

impl RemoteNodeBoundary {
    pub fn validate(&self) -> Result<(), RemoteBoundaryError> {
        let mut kinds = BTreeSet::new();
        for channel in &self.channels {
            if channel.maximum_message_bytes == 0 {
                return Err(RemoteBoundaryError::ZeroMessageLimit);
            }
            if !kinds.insert(channel.kind) {
                return Err(RemoteBoundaryError::DuplicateChannel);
            }
        }
        Ok(())
    }
}
