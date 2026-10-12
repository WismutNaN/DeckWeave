// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{NamespacedKind, ProviderId, SessionGeneration};
use serde::{Deserialize, Serialize};

/// Layout-independent identity used by automation bindings. OS scan codes and
/// virtual-key values are adapter details and never cross this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedKeyIdentity {
    pub key: NamespacedKind,
    pub location: KeyLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyLocation {
    Standard,
    Left,
    Right,
    Numpad,
    Unknown,
}

/// Repeat is a distinct observation rather than another pressed edge. This
/// prevents consumers from accidentally treating OS auto-repeat as a new hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyEdge {
    Pressed,
    Repeated,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum InputOrigin {
    /// The adapter cannot prove whether the OS record was hardware-generated.
    DeviceOrUnknown,
    DeckWeaveSynthetic {
        provider_id: ProviderId,
        runtime_generation: SessionGeneration,
    },
    OtherSynthetic,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputObservation {
    pub key: NormalizedKeyIdentity,
    pub edge: KeyEdge,
    pub origin: InputOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegeAvailability {
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivilegeStatus {
    pub capability: NamespacedKind,
    pub availability: PrivilegeAvailability,
    pub reason: Option<NamespacedKind>,
}
