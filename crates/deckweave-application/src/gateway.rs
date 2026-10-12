// SPDX-License-Identifier: GPL-3.0-or-later
//! Authorization, dispatch and stable error translation for application clients.

use crate::{
    ApiAuthScope, ApiAuthorizationError, ApiAuthorizer, ApiError, ApiErrorCode, AppCommand,
    AppCommandResult, AppQuery, AppQueryResult, ApplicationService, ApplyUseCase, CommandEnvelope,
    DraftMutationReceipt, EmptyRuntimeDiagnostics, ExecuteApplyRequest, HostSnapshot,
    OperationReceipt, PlanApplyRequest, RequestContext, RuntimeDiagnosticsSource,
};
use deckweave_domain::ids::PrincipalId;
use deckweave_workspace::{CommandClaim, CommandClaimRepository, DraftRepository, MutationKey};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

mod errors;
use errors::{api_error, map_apply_error, map_store_error};

pub trait HostStatusSource: Send + Sync {
    fn snapshot(&self) -> HostSnapshot;
}

pub struct ApplicationGateway {
    host: Arc<dyn HostStatusSource>,
    drafts: Arc<dyn DraftRepository>,
    command_claims: Arc<dyn CommandClaimRepository>,
    authorizer: Arc<dyn ApiAuthorizer>,
    apply: Arc<dyn ApplyUseCase>,
    diagnostics: Arc<dyn RuntimeDiagnosticsSource>,
}

impl ApplicationGateway {
    #[must_use]
    pub fn new(
        host: Arc<dyn HostStatusSource>,
        drafts: Arc<dyn DraftRepository>,
        command_claims: Arc<dyn CommandClaimRepository>,
        authorizer: Arc<dyn ApiAuthorizer>,
        apply: Arc<dyn ApplyUseCase>,
    ) -> Self {
        Self {
            host,
            drafts,
            command_claims,
            authorizer,
            apply,
            diagnostics: Arc::new(EmptyRuntimeDiagnostics),
        }
    }

    #[must_use]
    pub fn with_runtime_diagnostics(
        mut self,
        diagnostics: Arc<dyn RuntimeDiagnosticsSource>,
    ) -> Self {
        self.diagnostics = diagnostics;
        self
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
        match query {
            AppQuery::Host => {
                self.authorize(context, ApiAuthScope::Read)?;
                Ok(AppQueryResult::Host(self.snapshot()))
            }
            AppQuery::Draft { draft_id } => {
                self.authorize(context, ApiAuthScope::Read)?;
                self.drafts
                    .get_draft(&draft_id)
                    .map(AppQueryResult::Draft)
                    .map_err(|error| map_store_error(context, error))
            }
            AppQuery::PlanApply {
                draft_id,
                expected_draft_version,
                expected_active,
                allow_disruption,
            } => {
                self.authorize(context, ApiAuthScope::Simulation)?;
                self.apply
                    .plan_apply(&PlanApplyRequest {
                        draft_id,
                        expected_draft_version,
                        expected_active,
                        allow_disruption,
                    })
                    .map(AppQueryResult::ApplyPlan)
                    .map_err(|error| map_apply_error(context, error))
            }
            AppQuery::Operation { operation_id } => {
                self.authorize(context, ApiAuthScope::Read)?;
                self.apply
                    .operation(&operation_id)
                    .map(|snapshot| AppQueryResult::Operation(Box::new(snapshot)))
                    .map_err(|error| map_apply_error(context, error))
            }
            AppQuery::RuntimeTrace => {
                self.authorize(context, ApiAuthScope::Simulation)?;
                Ok(AppQueryResult::RuntimeTrace(Box::new(
                    self.diagnostics.runtime_trace(),
                )))
            }
        }
    }

    fn command(
        &self,
        context: &RequestContext,
        envelope: CommandEnvelope,
    ) -> Result<AppCommandResult, ApiError> {
        let required_scope = match &envelope.command {
            AppCommand::PatchDraft { .. } => ApiAuthScope::Draft,
            AppCommand::ApplyDraft { .. } => ApiAuthScope::Apply,
        };
        self.authorize(context, required_scope)?;
        if envelope.request_id.is_empty() || envelope.request_id.len() > 128 {
            return Err(api_error(
                context,
                ApiErrorCode::InvalidCommand,
                "api.error.invalid_request_id",
                false,
                BTreeMap::new(),
            ));
        }
        if !envelope.dry_run {
            let payload_fingerprint = serde_json::to_vec(&envelope.command)
                .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
                .map_err(|error| {
                    api_error(
                        context,
                        ApiErrorCode::Internal,
                        "api.error.command_fingerprint",
                        false,
                        BTreeMap::from([("reason".into(), error.to_string().into())]),
                    )
                })?;
            self.command_claims
                .claim_command(&CommandClaim {
                    key: MutationKey {
                        principal_id: context.principal_id.clone(),
                        dedup_epoch: envelope.dedup_epoch.clone(),
                        idempotency_key: envelope.idempotency_key.clone(),
                    },
                    payload_fingerprint,
                })
                .map_err(|error| map_store_error(context, error))?;
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
            AppCommand::ApplyDraft {
                draft_id,
                expected_draft_version,
                expected_active,
                allow_disruption,
                plan_hash,
            } => {
                if envelope.dry_run {
                    return Err(api_error(
                        context,
                        ApiErrorCode::InvalidCommand,
                        "api.error.apply_dry_run_requires_plan_query",
                        false,
                        BTreeMap::new(),
                    ));
                }
                self.apply
                    .execute_apply(&ExecuteApplyRequest {
                        principal_id: context.principal_id.clone(),
                        dedup_epoch: envelope.dedup_epoch,
                        idempotency_key: envelope.idempotency_key,
                        draft_id,
                        expected_draft_version,
                        expected_active,
                        allow_disruption,
                        plan_hash,
                    })
                    .map(|snapshot| {
                        AppCommandResult::OperationAccepted(OperationReceipt {
                            operation_id: snapshot.intent.operation_id,
                        })
                    })
                    .map_err(|error| map_apply_error(context, error))
            }
        }
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
