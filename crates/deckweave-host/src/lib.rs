// SPDX-License-Identifier: GPL-3.0-or-later
//! Долгоживущий host. В фазе 1 нет Store, API listener или hardware.

use deckweave_application::{
    ApiAuthScope, ApiAuthorizationError, ApiAuthorizer, ApiError, ApiErrorCode, AppCommand,
    AppCommandResult, AppQuery, AppQueryResult, ApplicationService, CommandEnvelope,
    DraftMutationReceipt, DraftStore, DraftStoreError, HostLease, HostPlatform, HostSnapshot,
    MutationKey, PlatformError, RequestContext,
};
use deckweave_domain::{LifecycleState, ids::PrincipalId};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

pub struct HostRuntime {
    platform: Arc<dyn HostPlatform>,
    _lease: Box<dyn HostLease>,
    state: Mutex<(LifecycleState, Option<String>)>,
    draft_store: Option<Arc<dyn DraftStore>>,
    api_authorizer: Arc<dyn ApiAuthorizer>,
}

impl HostRuntime {
    /// Блокировки приобретаются до появления runtime consumers.
    pub fn start(platform: Arc<dyn HostPlatform>) -> Result<Self, PlatformError> {
        let lease = platform.acquire()?;
        Ok(Self {
            platform,
            _lease: lease,
            state: Mutex::new((LifecycleState::Running, None)),
            draft_store: None,
            api_authorizer: Arc::new(DenyAllApiAuthorizer),
        })
    }

    pub fn start_with_services(
        platform: Arc<dyn HostPlatform>,
        draft_store: Arc<dyn DraftStore>,
        api_authorizer: Arc<dyn ApiAuthorizer>,
    ) -> Result<Self, PlatformError> {
        let lease = platform.acquire()?;
        Ok(Self {
            platform,
            _lease: lease,
            state: Mutex::new((LifecycleState::Running, None)),
            draft_store: Some(draft_store),
            api_authorizer,
        })
    }

    pub fn stop(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0 = LifecycleState::Stopping;
    }

    pub fn set_notice(&self, notice: Option<String>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1 = notice;
    }

    pub fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        self.platform.autostart_enabled()
    }

    pub fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError> {
        self.platform.set_autostart(enabled)
    }
}

impl ApplicationService for HostRuntime {
    fn snapshot(&self) -> HostSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        HostSnapshot {
            lifecycle: state.0,
            notice: state.1.clone(),
            mode: self.platform.mode(),
            data_root: self.platform.data_root().to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    fn query(&self, context: &RequestContext, query: AppQuery) -> Result<AppQueryResult, ApiError> {
        self.authorize(context, ApiAuthScope::Read)?;
        match query {
            AppQuery::Host => Ok(AppQueryResult::Host(self.snapshot())),
            AppQuery::Draft { draft_id } => self
                .draft_store(context)?
                .get_draft(&draft_id)
                .map(AppQueryResult::Draft)
                .map_err(|error| map_store_error(context, error)),
        }
    }

    fn command(
        &self,
        context: &RequestContext,
        envelope: CommandEnvelope,
    ) -> Result<AppCommandResult, ApiError> {
        self.authorize(context, ApiAuthScope::Draft)?;
        if envelope.request_id.is_empty() || envelope.request_id.len() > 128 {
            return Err(api_error(
                context,
                ApiErrorCode::InvalidCommand,
                "api.error.invalid_request_id",
                false,
                BTreeMap::new(),
            ));
        }
        let store = self.draft_store(context)?;
        match envelope.command {
            AppCommand::PatchDraft {
                draft_id,
                expected_version,
                edit,
            } => {
                if envelope.dry_run {
                    let draft = store
                        .preview_draft_mutation(&draft_id, expected_version, &edit)
                        .map_err(|error| map_store_error(context, error))?;
                    return Ok(AppCommandResult::DraftMutation {
                        draft: Box::new(draft),
                        receipt: None,
                        dry_run: true,
                    });
                }
                let stored = store
                    .mutate_draft(
                        &draft_id,
                        MutationKey {
                            principal_id: context.principal_id.clone(),
                            dedup_epoch: envelope.dedup_epoch,
                            idempotency_key: envelope.idempotency_key,
                        },
                        expected_version,
                        &edit,
                    )
                    .map_err(|error| map_store_error(context, error))?;
                let draft = store
                    .get_draft(&draft_id)
                    .map_err(|error| map_store_error(context, error))?;
                Ok(AppCommandResult::DraftMutation {
                    draft: Box::new(draft),
                    receipt: Some(DraftMutationReceipt {
                        key: stored.key,
                        before_version: stored.before_version,
                        after_version: stored.after_version,
                        document_sha256: stored.document_sha256,
                        replayed: stored.replayed,
                    }),
                    dry_run: false,
                })
            }
        }
    }
}

impl HostRuntime {
    fn authorize(&self, context: &RequestContext, scope: ApiAuthScope) -> Result<(), ApiError> {
        self.api_authorizer
            .authorize(&context.principal_id, scope)
            .map_err(|error| match error {
                ApiAuthorizationError::Denied => api_error(
                    context,
                    ApiErrorCode::PermissionDenied,
                    "api.error.permission_denied",
                    false,
                    BTreeMap::from([("requiredScope".into(), format!("{scope:?}").into())]),
                ),
                ApiAuthorizationError::Unavailable(message) => api_error(
                    context,
                    ApiErrorCode::Unavailable,
                    "api.error.authorization_unavailable",
                    true,
                    BTreeMap::from([("reason".into(), message.into())]),
                ),
            })
    }

    fn draft_store(&self, context: &RequestContext) -> Result<&dyn DraftStore, ApiError> {
        self.draft_store.as_deref().ok_or_else(|| {
            api_error(
                context,
                ApiErrorCode::Unavailable,
                "api.error.profile_store_unavailable",
                true,
                BTreeMap::new(),
            )
        })
    }
}

fn map_store_error(context: &RequestContext, error: DraftStoreError) -> ApiError {
    match error {
        DraftStoreError::NotFound => api_error(
            context,
            ApiErrorCode::NotFound,
            "api.error.draft_not_found",
            false,
            BTreeMap::new(),
        ),
        DraftStoreError::VersionConflict { expected, current } => api_error(
            context,
            ApiErrorCode::RevisionConflict,
            "api.error.revision_conflict",
            false,
            BTreeMap::from([
                ("expected".into(), expected.into()),
                ("current".into(), current.into()),
            ]),
        ),
        DraftStoreError::IdempotencyConflict => api_error(
            context,
            ApiErrorCode::IdempotencyConflict,
            "api.error.idempotency_conflict",
            false,
            BTreeMap::new(),
        ),
        DraftStoreError::InvalidCommand(message) => api_error(
            context,
            ApiErrorCode::InvalidCommand,
            "api.error.invalid_command",
            false,
            BTreeMap::from([("reason".into(), message.into())]),
        ),
        DraftStoreError::Unavailable(message) => api_error(
            context,
            ApiErrorCode::Unavailable,
            "api.error.profile_store_unavailable",
            true,
            BTreeMap::from([("reason".into(), message.into())]),
        ),
    }
}

fn api_error(
    context: &RequestContext,
    code: ApiErrorCode,
    message_key: &str,
    retryable: bool,
    details: BTreeMap<String, serde_json::Value>,
) -> ApiError {
    ApiError {
        code,
        message_key: message_key.into(),
        details,
        retryable,
        trace_id: context.trace_id.clone(),
    }
}

struct DenyAllApiAuthorizer;

impl ApiAuthorizer for DenyAllApiAuthorizer {
    fn authorize(
        &self,
        _principal_id: &PrincipalId,
        _scope: ApiAuthScope,
    ) -> Result<(), ApiAuthorizationError> {
        Err(ApiAuthorizationError::Denied)
    }
}

#[derive(Default)]
pub struct InMemoryApiAuthorizer {
    scopes: Mutex<BTreeMap<PrincipalId, BTreeSet<ApiAuthScope>>>,
}

impl InMemoryApiAuthorizer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&self, principal_id: PrincipalId, scopes: impl IntoIterator<Item = ApiAuthScope>) {
        self.scopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(principal_id, scopes.into_iter().collect());
    }

    pub fn revoke(&self, principal_id: &PrincipalId) {
        self.scopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(principal_id);
    }
}

impl ApiAuthorizer for InMemoryApiAuthorizer {
    fn authorize(
        &self,
        principal_id: &PrincipalId,
        scope: ApiAuthScope,
    ) -> Result<(), ApiAuthorizationError> {
        let scopes = self
            .scopes
            .lock()
            .map_err(|_| ApiAuthorizationError::Unavailable("scope lock is poisoned".into()))?;
        if scopes
            .get(principal_id)
            .is_some_and(|scopes| scopes.contains(&scope) || scopes.contains(&ApiAuthScope::Admin))
        {
            Ok(())
        } else {
            Err(ApiAuthorizationError::Denied)
        }
    }
}
