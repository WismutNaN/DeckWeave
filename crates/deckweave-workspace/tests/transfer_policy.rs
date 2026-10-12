// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{SnapshotId, WorkspaceId, WorkspaceRevisionId};
use deckweave_workspace::{
    ExportSelection, ExportSelectionError, RevisionExportSelection, SnapshotRef,
    SnapshotValidationError, validate_export_selection, validate_snapshot,
};
use std::{collections::BTreeMap, error::Error};

fn revision() -> Result<RevisionExportSelection, Box<dyn Error>> {
    Ok(RevisionExportSelection {
        workspace_id: WorkspaceId::try_new("workspace-transfer")?,
        revision_id: WorkspaceRevisionId::try_new("revision-transfer")?,
    })
}

#[test]
fn export_selection_rejects_empty_and_duplicate_revisions() -> Result<(), Box<dyn Error>> {
    let empty = ExportSelection {
        revisions: vec![],
        include_assets: false,
        include_private_sections: false,
    };
    assert_eq!(
        validate_export_selection(&empty),
        Err(ExportSelectionError::Empty)
    );
    let selected = revision()?;
    let duplicate = ExportSelection {
        revisions: vec![selected.clone(), selected],
        include_assets: true,
        include_private_sections: true,
    };
    assert_eq!(
        validate_export_selection(&duplicate),
        Err(ExportSelectionError::DuplicateRevision)
    );
    Ok(())
}

#[test]
fn snapshot_policy_is_independent_from_filesystem_storage() -> Result<(), Box<dyn Error>> {
    let mut snapshot = SnapshotRef {
        schema_version: 1,
        snapshot_id: SnapshotId::try_new("snapshot-transfer")?,
        workspace_id: WorkspaceId::try_new("workspace-transfer")?,
        revision_id: WorkspaceRevisionId::try_new("revision-transfer")?,
        name: "Before redesign".into(),
        comment: None,
        created_at_unix_ms: 1,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    };
    validate_snapshot(&snapshot)?;
    snapshot.name = " ".into();
    assert_eq!(
        validate_snapshot(&snapshot),
        Err(SnapshotValidationError::InvalidName)
    );
    Ok(())
}
