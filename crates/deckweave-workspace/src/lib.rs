// SPDX-License-Identifier: GPL-3.0-or-later
//! Workspace Configuration bounded context.

mod apply;
mod binding;
mod candidate;
mod command;
mod model;
mod plugin_instance;
pub mod plugin_services;
pub use plugin_instance::PluginInstanceConfiguration;
mod repository;
mod snapshot;
mod transfer;
mod validation;

pub use binding::EndpointBinding;
pub use command::{DraftEditError, apply_draft_command, preview_draft_mutation};
pub use model::{
    AssetReference, DraftCommand, PackageLockEntry, WorkspaceDraft, WorkspaceRevision,
};
pub use repository::{
    CommandClaim, CommandClaimOutcome, CommandClaimRepository, DraftRepository,
    DraftRepositoryError, DraftView, MutationKey, MutationWindow, StoredDraftMutation,
};
pub use snapshot::{SnapshotRef, SnapshotValidationError, validate_snapshot};
pub use transfer::{
    ExportSelection, ExportSelectionError, ImportInspection, ImportRevisionSummary,
    RevisionExportSelection, validate_export_selection,
};
pub use validation::{
    WorkspaceValidationError, validate_draft, validate_revision, validate_workspace_content,
};

// Temporary compatibility names while Application Gateway consumers migrate.
pub use apply::{
    ActiveSelection, ApplyBegin, ApplyCommitRecord, ApplyCommitStatus, ApplyOperationIntent,
    ApplyOperationRepository, ApplyOperationSnapshot, ApplyOperationState, ApplyRepositoryError,
    ApplyRuntimeHealth,
};
pub use candidate::{CandidateError, WorkspaceCandidate};
pub use repository::{DraftRepository as DraftStore, DraftRepositoryError as DraftStoreError};
