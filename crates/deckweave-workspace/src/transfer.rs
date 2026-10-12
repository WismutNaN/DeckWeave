// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{AssetId, CredentialRef, WorkspaceId, WorkspaceRevisionId};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RevisionExportSelection {
    pub workspace_id: WorkspaceId,
    pub revision_id: WorkspaceRevisionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSelection {
    pub revisions: Vec<RevisionExportSelection>,
    pub include_assets: bool,
    pub include_private_sections: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExportSelectionError {
    #[error("export selection is empty")]
    Empty,
    #[error("export selection contains duplicate revisions")]
    DuplicateRevision,
}

pub fn validate_export_selection(selection: &ExportSelection) -> Result<(), ExportSelectionError> {
    if selection.revisions.is_empty() {
        return Err(ExportSelectionError::Empty);
    }
    let unique: BTreeSet<_> = selection.revisions.iter().collect();
    if unique.len() != selection.revisions.len() {
        return Err(ExportSelectionError::DuplicateRevision);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRevisionSummary {
    pub workspace_id: WorkspaceId,
    pub source_revision_id: WorkspaceRevisionId,
    pub has_private_section: bool,
    pub package_count: usize,
    pub asset_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportInspection {
    pub revisions: Vec<ImportRevisionSummary>,
    pub included_assets: Vec<AssetId>,
    pub missing_assets: Vec<AssetId>,
    pub unresolved_credential_refs: Vec<CredentialRef>,
}
