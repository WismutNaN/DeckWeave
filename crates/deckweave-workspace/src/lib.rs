// SPDX-License-Identifier: GPL-3.0-or-later
//! Workspace Configuration bounded context.

mod command;
mod model;
mod repository;
mod validation;

pub use command::{DraftEditError, apply_draft_command, preview_draft_mutation};
pub use model::{
    AssetReference, DraftCommand, PackageLockEntry, WorkspaceDraft, WorkspaceRevision,
};
pub use repository::{
    DraftRepository, DraftRepositoryError, DraftView, MutationKey, StoredDraftMutation,
};
pub use validation::{
    WorkspaceValidationError, validate_draft, validate_revision, validate_workspace_content,
};

// Temporary compatibility names while Application Gateway consumers migrate.
pub use repository::{DraftRepository as DraftStore, DraftRepositoryError as DraftStoreError};
