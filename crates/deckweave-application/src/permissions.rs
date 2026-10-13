// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit, manifest-pinned grants; installing a package never grants privileges.
use crate::{
    ApiAuthScope, ApiAuthorizer, ApiError, ApiErrorCode, RequestContext, ScopedAuthorizationStore,
};
use deckweave_domain::{
    ids::{PrincipalId, RuntimeGroupId},
    security::PermissionRequest,
};
use deckweave_platform::{
    PermissionPreferenceStore, PermissionPreferences, PermissionSelection, PreferenceError,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOffer {
    pub selection: PermissionSelection,
    pub request: PermissionRequest,
    pub runtime_group_ids: Vec<RuntimeGroupId>,
}
pub trait PermissionCatalogSource: Send + Sync {
    fn permission_offers(&self, principal: &PrincipalId) -> Result<Vec<PermissionOffer>, String>;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSnapshot {
    pub preferences: PermissionPreferences,
    pub offers: Vec<PermissionOffer>,
}

pub struct PluginPermissionService {
    store: Arc<dyn PermissionPreferenceStore>,
    offers: Vec<PermissionOffer>,
    principal: PrincipalId,
    grants: Arc<ScopedAuthorizationStore>,
    authorizer: Arc<dyn ApiAuthorizer>,
    synchronized: Mutex<Option<PermissionPreferences>>,
}

impl PluginPermissionService {
    pub fn new(
        store: Arc<dyn PermissionPreferenceStore>,
        source: &dyn PermissionCatalogSource,
        principal: PrincipalId,
        grants: Arc<ScopedAuthorizationStore>,
        authorizer: Arc<dyn ApiAuthorizer>,
    ) -> Result<Self, String> {
        let offers = source.permission_offers(&principal)?;
        if offers.len() > 4096 {
            return Err("permission catalog budget".into());
        }
        let service = Self {
            store,
            offers,
            principal,
            grants,
            authorizer,
            synchronized: Mutex::new(None),
        };
        let mut state = service.synchronized.lock().map_err(|e| e.to_string())?;
        // A damaged grant file disables privileges but must not prevent the editor opening.
        let _ = service.synchronize(&mut state);
        drop(state);
        Ok(service)
    }

    pub fn get(&self, context: &RequestContext) -> Result<PermissionSnapshot, ApiError> {
        self.authorize(context, ApiAuthScope::Read)?;
        let mut state = self
            .synchronized
            .lock()
            .map_err(|_| api_error(context, PreferenceError::Unavailable("lock".into())))?;
        let preferences = self
            .synchronize(&mut state)
            .map_err(|e| api_error(context, e))?;
        Ok(PermissionSnapshot {
            preferences,
            offers: self.offers.clone(),
        })
    }

    pub fn change(
        &self,
        context: &RequestContext,
        expected: u64,
        selected: Vec<PermissionSelection>,
    ) -> Result<PermissionSnapshot, ApiError> {
        self.authorize(context, ApiAuthScope::Preferences)?;
        let mut state = self
            .synchronized
            .lock()
            .map_err(|_| api_error(context, PreferenceError::Unavailable("lock".into())))?;
        let next = PermissionPreferences {
            revision: expected
                .checked_add(1)
                .ok_or_else(|| api_error(context, PreferenceError::Invalid))?,
            selected,
        };
        next.validate().map_err(|e| api_error(context, e))?;
        if next
            .selected
            .iter()
            .any(|s| !self.offers.iter().any(|o| o.selection == *s))
        {
            return Err(api_error(context, PreferenceError::Invalid));
        }
        let written = self.store.compare_exchange(expected, &next);
        // Reconcile even an uncertain publication before reporting an error to the UI.
        let preferences = self
            .synchronize(&mut state)
            .map_err(|e| api_error(context, e))?;
        written.map_err(|e| api_error(context, e))?;
        Ok(PermissionSnapshot {
            preferences,
            offers: self.offers.clone(),
        })
    }

    fn authorize(&self, context: &RequestContext, scope: ApiAuthScope) -> Result<(), ApiError> {
        if context.principal_id != self.principal
            || self
                .authorizer
                .authorize(&context.principal_id, scope)
                .is_err()
        {
            return Err(crate::gateway::errors::api_error(
                context,
                ApiErrorCode::PermissionDenied,
                "api.error.permission_denied",
                false,
                Default::default(),
            ));
        }
        Ok(())
    }

    fn synchronize(
        &self,
        state: &mut Option<PermissionPreferences>,
    ) -> Result<PermissionPreferences, PreferenceError> {
        let stored = match self.store.load() {
            Ok(value) => value,
            Err(error) => {
                let _ = self.grants.replace(vec![]);
                *state = None;
                return Err(error);
            }
        };
        if state.as_ref() != Some(&stored) {
            let requests = self
                .offers
                .iter()
                .filter(|o| stored.selected.contains(&o.selection))
                .map(|o| o.request.clone())
                .collect();
            self.grants
                .replace(requests)
                .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
            *state = Some(stored.clone());
        }
        // Stale package hashes remain stored for review but never acquire a grant.
        Ok(stored)
    }
}
fn api_error(context: &RequestContext, error: PreferenceError) -> ApiError {
    let (code, key) = match error {
        PreferenceError::Conflict => (
            ApiErrorCode::RevisionConflict,
            "api.error.permissions_conflict",
        ),
        PreferenceError::Invalid => (
            ApiErrorCode::InvalidCommand,
            "api.error.invalid_permissions",
        ),
        PreferenceError::Unavailable(_) => (
            ApiErrorCode::Unavailable,
            "api.error.permissions_unavailable",
        ),
    };
    crate::gateway::errors::api_error(context, code, key, false, Default::default())
}
