// SPDX-License-Identifier: GPL-3.0-or-later
//! Workspace repository port implemented by the JSON profile adapter.

use super::*;
use deckweave_workspace::{
    DraftRepository, DraftRepositoryError, DraftView, MutationWindow, StoredDraftMutation,
};

impl DraftRepository for JsonProfileStore {
    fn open_mutation_window(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<MutationWindow, DraftRepositoryError> {
        JsonProfileStore::open_mutation_window(self, principal_id)
            .map_err(map_application_store_error)
    }

    fn get_draft(&self, draft_id: &DraftId) -> Result<DraftView, DraftRepositoryError> {
        let draft = self
            .load_draft(draft_id)
            .map_err(map_application_store_error)?
            .draft;
        Ok(draft_view(draft))
    }

    fn preview_draft_mutation(
        &self,
        draft_id: &DraftId,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<DraftView, DraftRepositoryError> {
        JsonProfileStore::preview_draft_mutation(self, draft_id, expected_version, command)
            .map(draft_view)
            .map_err(map_application_store_error)
    }

    fn mutate_draft(
        &self,
        draft_id: &DraftId,
        key: MutationKey,
        expected_version: DraftVersion,
        command: &DraftCommand,
    ) -> Result<StoredDraftMutation, DraftRepositoryError> {
        let outcome =
            JsonProfileStore::mutate_draft(self, draft_id, key, expected_version, command)
                .map_err(map_application_store_error)?;
        Ok(StoredDraftMutation {
            draft: draft_view(outcome.draft),
            key: outcome.receipt.key,
            before_version: outcome.receipt.before_version,
            after_version: outcome.receipt.after_version,
            document_sha256: outcome.receipt.document_sha256,
            replayed: outcome.replayed,
        })
    }
}

fn draft_view(draft: WorkspaceDraft) -> DraftView {
    DraftView {
        draft_id: draft.draft_id,
        workspace_id: draft.workspace_id,
        base_revision_id: draft.base_revision_id,
        version: draft.version,
        document: draft.document,
    }
}

fn map_application_store_error(error: DraftMutationError) -> DraftRepositoryError {
    match error {
        DraftMutationError::Store(ProfileStoreError::NotFound) => DraftRepositoryError::NotFound,
        DraftMutationError::Store(ProfileStoreError::InvalidDocument(message))
        | DraftMutationError::InvalidCommand(message) => {
            DraftRepositoryError::InvalidCommand(message)
        }
        DraftMutationError::VersionConflict { expected, current } => {
            DraftRepositoryError::VersionConflict { expected, current }
        }
        DraftMutationError::IdempotencyConflict => DraftRepositoryError::IdempotencyConflict,
        DraftMutationError::IdempotencyExpired => DraftRepositoryError::IdempotencyExpired,
        DraftMutationError::ReceiptQuota => DraftRepositoryError::ReceiptQuota,
        other => DraftRepositoryError::Unavailable(other.to_string()),
    }
}
