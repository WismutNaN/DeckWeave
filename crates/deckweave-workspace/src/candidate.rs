// SPDX-License-Identifier: GPL-3.0-or-later
//! Immutable, validated input handed from Workspace Configuration to planning.

use crate::{
    AssetReference, PackageLockEntry, WorkspaceDraft, WorkspaceValidationError, validate_draft,
};
use deckweave_domain::ids::{DraftId, DraftVersion, WorkspaceId, WorkspaceRevisionId};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceCandidate {
    draft: WorkspaceDraft,
    content_sha256: String,
    package_lock_sha256: String,
    assets_sha256: String,
}

#[derive(Debug, Error)]
pub enum CandidateError {
    #[error(transparent)]
    Invalid(#[from] WorkspaceValidationError),
    #[error("candidate serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl WorkspaceCandidate {
    pub fn from_draft(draft: WorkspaceDraft) -> Result<Self, CandidateError> {
        validate_draft(&draft)?;
        let content_sha256 = digest(&canonical_json(&draft)?);
        let package_lock_sha256 = digest(&canonical_json(&draft.package_lock)?);
        let assets_sha256 = digest(&canonical_json(&draft.assets)?);
        Ok(Self {
            draft,
            content_sha256,
            package_lock_sha256,
            assets_sha256,
        })
    }

    #[must_use]
    pub fn draft_id(&self) -> &DraftId {
        &self.draft.draft_id
    }

    #[must_use]
    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.draft.workspace_id
    }

    #[must_use]
    pub fn base_revision_id(&self) -> Option<&WorkspaceRevisionId> {
        self.draft.base_revision_id.as_ref()
    }

    #[must_use]
    pub fn version(&self) -> DraftVersion {
        self.draft.version
    }

    #[must_use]
    pub fn package_lock(&self) -> &[PackageLockEntry] {
        &self.draft.package_lock
    }

    #[must_use]
    pub fn document(&self) -> &serde_json::Value {
        &self.draft.document
    }

    #[must_use]
    pub fn assets(&self) -> &[AssetReference] {
        &self.draft.assets
    }

    #[must_use]
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    #[must_use]
    pub fn package_lock_sha256(&self) -> &str {
        &self.package_lock_sha256
    }

    #[must_use]
    pub fn assets_sha256(&self) -> &str {
        &self.assets_sha256
    }
}

fn canonical_json(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(value)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
