// SPDX-License-Identifier: GPL-3.0-or-later
//! Authorization, dispatch and stable error translation for application clients.

use crate::{
    ApiAuthScope, ApiAuthorizationError, ApiAuthorizer, ApiError, ApiErrorCode, AppCommand,
    AppCommandResult, AppQuery, AppQueryResult, ApplicationService, CommandEnvelope,
    DraftMutationReceipt, HostSnapshot, RequestContext,
};
use deckweave_domain::ids::PrincipalId;
use deckweave_workspace::{DraftRepository, DraftRepositoryError, MutationKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

pub trait HostStatusSource: Send + Sync {
    fn snapshot(&self) -> HostSnapshot;
}

pub struct ApplicationGateway {
    host: Arc<dyn HostStatusSource>,
    drafts: Arc<dyn DraftRepository>,
    authorizer: Arc<dyn ApiAuthorizer>,
}

impl ApplicationGateway {
    #[must_use]
    pub fn new(
        host: Arc<dyn HostStatusSource>,
        drafts: Arc<dyn DraftRepository>,
        authorizer: Arc<dyn ApiAuthorizer>,
    ) -> Self {
        Self {
            host,
            drafts,
            authorizer,
        }
    }

    fn authorize(&self, context: &RequestContext, scope: ApiAuthScope) -> Result<(), ApiError> {
        self.authorizer
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
}

impl ApplicationService for ApplicationGateway {
    fn snapshot(&self) -> HostSnapshot {
        self.host.snapshot()
    }

    fn open_mutation_window(
        &self,
        context: &RequestContext,
    ) -> Result<crate::MutationWindow, ApiError> {
        self.authorize(context, ApiAuthScope::Draft)?;
        self.drafts
            .open_mutation_window(&context.principal_id)
            .map_err(|error| map_store_error(context, error))
    }

    fn query(&self, context: &RequestContext, query: AppQuery) -> Result<AppQueryResult, ApiError> {
        self.authorize(context, ApiAuthScope::Read)?;
        match query {
            AppQuery::Host => Ok(AppQueryResult::Host(self.snapshot())),
            AppQuery::Draft { draft_id } => self
                .drafts
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
        match envelope.command {
            AppCommand::PatchDraft {
                draft_id,
                expected_version,
                edit,
            } => {
                if envelope.dry_run {
                    let draft = self
                        .drafts
                        .preview_draft_mutation(&draft_id, expected_version, &edit)
                        .map_err(|error| map_store_error(context, error))?;
                    return Ok(AppCommandResult::DraftMutation {
                        draft: Box::new(draft),
                        receipt: None,
                        dry_run: true,
                    });
                }
                let stored = self
                    .drafts
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
                Ok(AppCommandResult::DraftMutation {
                    draft: Box::new(stored.draft),
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

fn map_store_error(context: &RequestContext, error: DraftRepositoryError) -> ApiError {
    match error {
        DraftRepositoryError::NotFound => api_error(
            context,
            ApiErrorCode::NotFound,
            "api.error.draft_not_found",
            false,
            BTreeMap::new(),
        ),
        DraftRepositoryError::VersionConflict { expected, current } => api_error(
            context,
            ApiErrorCode::RevisionConflict,
            "api.error.revision_conflict",
            false,
            BTreeMap::from([
                ("expected".into(), expected.into()),
                ("current".into(), current.into()),
            ]),
        ),
        DraftRepositoryError::IdempotencyConflict => api_error(
            context,
            ApiErrorCode::IdempotencyConflict,
            "api.error.idempotency_conflict",
            false,
            BTreeMap::new(),
        ),
        DraftRepositoryError::IdempotencyExpired => api_error(
            context,
            ApiErrorCode::IdempotencyExpired,
            "api.error.idempotency_expired",
            false,
            BTreeMap::new(),
        ),
        DraftRepositoryError::ReceiptQuota => api_error(
            context,
            ApiErrorCode::CapacityExceeded,
            "api.error.mutation_receipt_quota",
            false,
            BTreeMap::new(),
        ),
        DraftRepositoryError::InvalidCommand(message) => api_error(
            context,
            ApiErrorCode::InvalidCommand,
            "api.error.invalid_command",
            false,
            BTreeMap::from([("reason".into(), message.into())]),
        ),
        DraftRepositoryError::Unavailable(message) => api_error(
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
