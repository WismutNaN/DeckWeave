// SPDX-License-Identifier: GPL-3.0-or-later
//! Contracts for observing, intercepting and emitting platform input.

mod capture;
mod observation;

pub use capture::{
    CaptureMode, CapturePolicy, InputContractError, InputEmitterDescriptor, InputGranularity,
    InputSourceDescriptor, validate_capture, validate_privilege, validate_source,
};
pub use observation::{
    InputObservation, InputOrigin, KeyEdge, KeyLocation, NormalizedKeyIdentity,
    PrivilegeAvailability, PrivilegeStatus,
};
