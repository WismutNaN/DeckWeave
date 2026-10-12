// SPDX-License-Identifier: GPL-3.0-or-later
//! CAS host preference use case. A failed notification does not undo durable settings.
use crate::{ApiAuthScope, ApiAuthorizer, ApiError, ApiErrorCode, RequestContext};
use deckweave_domain::localization::LocaleContext;
use deckweave_platform::{LocalePreferenceStore, PreferenceError};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub trait LocalePublisher: Send + Sync {
    fn publish_locale(&self, context: LocaleContext) -> Result<(), String>;
}
#[derive(Debug, Error)]
pub enum LocaleError {
    #[error("locale preference access denied")]
    Denied,
    #[error(transparent)]
    Store(#[from] PreferenceError),
}
impl LocaleError {
    pub fn into_api_error(self, context: &RequestContext) -> ApiError {
        let (code, key, retryable) = match self {
            Self::Denied => (
                ApiErrorCode::PermissionDenied,
                "api.error.permission_denied",
                false,
            ),
            Self::Store(PreferenceError::Conflict) => (
                ApiErrorCode::RevisionConflict,
                "api.error.locale_conflict",
                false,
            ),
            Self::Store(PreferenceError::Invalid) => (
                ApiErrorCode::InvalidCommand,
                "api.error.invalid_locale",
                false,
            ),
            Self::Store(PreferenceError::Unavailable(_)) => (
                ApiErrorCode::Unavailable,
                "api.error.locale_unavailable",
                true,
            ),
        };
        crate::gateway::errors::api_error(context, code, key, retryable, Default::default())
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleUpdate {
    pub context: LocaleContext,
    pub delivery_pending: bool,
}
pub struct LocalePreferencesService {
    store: Arc<dyn LocalePreferenceStore>,
    publisher: Arc<dyn LocalePublisher>,
    authorizer: Arc<dyn ApiAuthorizer>,
    order: Mutex<()>,
}
impl LocalePreferencesService {
    pub fn new(
        store: Arc<dyn LocalePreferenceStore>,
        publisher: Arc<dyn LocalePublisher>,
        authorizer: Arc<dyn ApiAuthorizer>,
    ) -> Self {
        Self {
            store,
            publisher,
            authorizer,
            order: Mutex::new(()),
        }
    }
    pub fn get(&self, request: &RequestContext) -> Result<LocaleContext, LocaleError> {
        self.authorizer
            .authorize(&request.principal_id, ApiAuthScope::Read)
            .map_err(|_| LocaleError::Denied)?;
        Ok(self.store.load()?)
    }
    pub fn change(
        &self,
        request: &RequestContext,
        expected: u64,
        next: LocaleContext,
    ) -> Result<LocaleUpdate, LocaleError> {
        self.authorizer
            .authorize(&request.principal_id, ApiAuthScope::Preferences)
            .map_err(|_| LocaleError::Denied)?;
        let _guard = self
            .order
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        self.store.compare_exchange(expected, &next)?;
        let delivery_pending = self.publisher.publish_locale(next.clone()).is_err();
        Ok(LocaleUpdate {
            context: next,
            delivery_pending,
        })
    }
    /// Retry only the currently stored revision; never overwrite a newer selection.
    pub fn reconcile(&self, request: &RequestContext) -> Result<LocaleUpdate, LocaleError> {
        self.authorizer
            .authorize(&request.principal_id, ApiAuthScope::Preferences)
            .map_err(|_| LocaleError::Denied)?;
        let _guard = self
            .order
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        let context = self.store.load()?;
        let delivery_pending = self.publisher.publish_locale(context.clone()).is_err();
        Ok(LocaleUpdate {
            context,
            delivery_pending,
        })
    }
}
