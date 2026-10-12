// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-managed exact grants. Installation and profile import never call replace.
use crate::{AuthorizationError, AuthorizationStore};
use deckweave_domain::{
    ids::{GrantId, GrantRevision},
    security::{PermissionGrant, PermissionRequest},
};
use std::sync::RwLock;

pub struct ScopedAuthorizationStore {
    state: RwLock<(u64, Vec<PermissionRequest>)>,
}

impl Default for ScopedAuthorizationStore {
    fn default() -> Self {
        Self {
            state: RwLock::new((1, vec![])),
        }
    }
}

impl ScopedAuthorizationStore {
    /// Called only by a host-authorized grant-management use case. A whole revision
    /// changes atomically; existing runtime dispatch revalidates that revision.
    pub fn replace(
        &self,
        requests: Vec<PermissionRequest>,
    ) -> Result<GrantRevision, AuthorizationError> {
        if requests.len() > 4096 {
            return Err(AuthorizationError::Unavailable("grant budget".into()));
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| AuthorizationError::Unavailable("grant lock".into()))?;
        state.0 = state
            .0
            .checked_add(1)
            .ok_or_else(|| AuthorizationError::Unavailable("grant revision exhausted".into()))?;
        state.1 = requests;
        GrantRevision::try_new(state.0).map_err(|e| AuthorizationError::Unavailable(e.to_string()))
    }
}

impl AuthorizationStore for ScopedAuthorizationStore {
    fn revision(&self) -> Result<GrantRevision, AuthorizationError> {
        let state = self
            .state
            .read()
            .map_err(|_| AuthorizationError::Unavailable("grant lock".into()))?;
        GrantRevision::try_new(state.0).map_err(|e| AuthorizationError::Unavailable(e.to_string()))
    }
    fn authorize(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionGrant, AuthorizationError> {
        let state = self
            .state
            .read()
            .map_err(|_| AuthorizationError::Unavailable("grant lock".into()))?;
        let index = state
            .1
            .iter()
            .position(|grant| {
                grant.principal_id == request.principal_id
                    && grant.package_id == request.package_id
                    && grant.contribution_id == request.contribution_id
                    && grant.scope == request.scope
                    && grant.resource_selector == request.resource_selector
                    && grant.access_mode == request.access_mode
                    && grant.extensions == request.extensions
            })
            .ok_or(AuthorizationError::Denied)?;
        Ok(PermissionGrant {
            grant_id: GrantId::try_new(format!("grant-{}-{index}", state.0))
                .map_err(|e| AuthorizationError::Unavailable(e.to_string()))?,
            revision: GrantRevision::try_new(state.0)
                .map_err(|e| AuthorizationError::Unavailable(e.to_string()))?,
            request: request.clone(),
            expires_at_rfc3339: None,
            extensions: Default::default(),
        })
    }
}
