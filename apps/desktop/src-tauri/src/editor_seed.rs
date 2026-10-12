// SPDX-License-Identifier: GPL-3.0-or-later
//! Первый черновик создаётся явно при открытии editor, не при чтении snapshot.
use deckweave_domain::ids::{DraftId, DraftVersion, WorkspaceId};
use deckweave_profiles::{JsonProfileStore, ProfileStore};
use deckweave_workspace::{ApplyOperationRepository, WorkspaceDraft};
use std::error::Error;

pub fn initial(store: &JsonProfileStore) -> Result<WorkspaceDraft, Box<dyn Error>> {
    let mut draft = WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("desktop-main")?,
        workspace_id: WorkspaceId::try_new("desktop-workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({"schemaVersion":{"major":1,"minor":0},"graph":{"nodes":[],"edges":[],"extensions":{}},"layout":{"nodes":{},"extensions":{}},"extensions":{}}),
        assets: vec![],
        private_section: None,
        extensions: Default::default(),
        unknown_fields: Default::default(),
    };
    if let Some(commit) = store.active_commit()? {
        let revision = store.load_revision(
            &commit.candidate.workspace_id,
            &commit.candidate.revision_id,
        )?;
        draft.workspace_id = revision.workspace_id;
        draft.base_revision_id = Some(revision.revision_id);
        draft.document = revision.document;
        draft.package_lock = revision.package_lock;
        draft.assets = revision.assets;
        draft.private_section = revision.private_section;
        draft.extensions = revision.extensions;
        draft.unknown_fields = revision.unknown_fields;
    }
    Ok(draft)
}
