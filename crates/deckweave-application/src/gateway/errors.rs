// SPDX-License-Identifier: GPL-3.0-or-later
//! Stable translation from bounded-context errors to application API errors.

use crate::{ApiError, ApiErrorCode, ApplyUseCaseError, RequestContext};
use deckweave_workspace::DraftRepositoryError;
use std::collections::BTreeMap;

pub(super) fn map_apply_error(context: &RequestContext, error: ApplyUseCaseError) -> ApiError {
    match error {
        ApplyUseCaseError::NotFound => api_error(
            context,
            ApiErrorCode::NotFound,
            "api.error.apply_not_found",
            false,
            BTreeMap::new(),
        ),
        ApplyUseCaseError::PreconditionChanged(reason) => api_error(
            context,
            ApiErrorCode::RevisionConflict,
            "api.error.apply_precondition_changed",
            false,
            BTreeMap::from([("reason".into(), reason.into())]),
        ),
        ApplyUseCaseError::InvalidPlan(reason) => api_error(
            context,
            ApiErrorCode::PlanConflict,
            "api.error.apply_plan_conflict",
            false,
            BTreeMap::from([("reason".into(), reason.into())]),
        ),
        ApplyUseCaseError::IdempotencyConflict => api_error(
            context,
            ApiErrorCode::IdempotencyConflict,
            "api.error.idempotency_conflict",
            false,
            BTreeMap::new(),
        ),
        ApplyUseCaseError::IdempotencyExpired => api_error(
            context,
            ApiErrorCode::IdempotencyExpired,
            "api.error.idempotency_expired",
            false,
            BTreeMap::new(),
        ),
        ApplyUseCaseError::CapacityExceeded => api_error(
            context,
            ApiErrorCode::CapacityExceeded,
            "api.error.apply_operation_quota",
            false,
            BTreeMap::new(),
        ),
        ApplyUseCaseError::RecoveryRequired(reason) => api_error(
            context,
            ApiErrorCode::RecoveryRequired,
            "api.error.apply_recovery_required",
            true,
            BTreeMap::from([("reason".into(), reason.into())]),
        ),
        ApplyUseCaseError::Unavailable(reason) => api_error(
            context,
            ApiErrorCode::Unavailable,
            "api.error.apply_unavailable",
            true,
            BTreeMap::from([("reason".into(), reason.into())]),
        ),
    }
}

pub(super) fn map_store_error(context: &RequestContext, error: DraftRepositoryError) -> ApiError {
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
        DraftRepositoryError::InvalidCandidate(message) => api_error(
            context,
            ApiErrorCode::InvalidCommand,
            "api.error.invalid_candidate",
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

pub(super) fn api_error(
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
