// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal authorization adapter for packages that request no permissions.

use crate::{AuthorizationError, AuthorizationStore};
use deckweave_domain::{
    ids::GrantRevision,
    security::{PermissionGrant, PermissionRequest},
};

#[derive(Debug, Default)]
pub struct DenyAllAuthorizationStore;

impl AuthorizationStore for DenyAllAuthorizationStore {
    fn revision(&self) -> Result<GrantRevision, AuthorizationError> {
        GrantRevision::try_new(1)
            .map_err(|error| AuthorizationError::Unavailable(error.to_string()))
    }

    fn authorize(
        &self,
        _request: &PermissionRequest,
    ) -> Result<PermissionGrant, AuthorizationError> {
        Err(AuthorizationError::Denied)
    }
}
