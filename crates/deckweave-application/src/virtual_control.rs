// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit live virtual input; unlike simulation this can execute applied actions.
use deckweave_domain::{
    endpoint::StableEndpointRef,
    ids::{ActivationEpoch, SessionGeneration},
};
use deckweave_workspace::ActiveSelection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VirtualClickRequest {
    pub endpoint: StableEndpointRef,
    pub expected_active: ActiveSelection,
    pub activation_epoch: ActivationEpoch,
    pub session_generation: SessionGeneration,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDisplayValue {
    pub endpoint: StableEndpointRef,
    pub text: String,
}
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualSurfaceSnapshot {
    pub activation_epoch: Option<ActivationEpoch>,
    pub displays: Vec<VirtualDisplayValue>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualControlError {
    Stale,
    Unsupported,
    Unavailable,
}
pub trait VirtualControlPort: Send + Sync {
    fn snapshot(&self) -> Result<VirtualSurfaceSnapshot, VirtualControlError>;
    fn click(&self, request: &VirtualClickRequest) -> Result<(), VirtualControlError>;
}
pub struct UnavailableVirtualControl;
impl VirtualControlPort for UnavailableVirtualControl {
    fn snapshot(&self) -> Result<VirtualSurfaceSnapshot, VirtualControlError> {
        Ok(VirtualSurfaceSnapshot::default())
    }
    fn click(&self, _: &VirtualClickRequest) -> Result<(), VirtualControlError> {
        Err(VirtualControlError::Unavailable)
    }
}
