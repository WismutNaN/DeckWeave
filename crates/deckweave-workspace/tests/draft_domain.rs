// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{DraftId, DraftVersion, WorkspaceId};
use deckweave_workspace::{DraftCommand, DraftEditError, WorkspaceDraft, preview_draft_mutation};
use std::{collections::BTreeMap, error::Error};

fn draft() -> Result<WorkspaceDraft, Box<dyn Error>> {
    Ok(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("draft-domain")?,
        workspace_id: WorkspaceId::try_new("workspace-domain")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({"nested": {"title/value": "Before"}, "items": [1]}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
}

#[test]
fn command_uses_json_pointer_and_advances_version_once() -> Result<(), Box<dyn Error>> {
    let original = draft()?;
    let changed = preview_draft_mutation(
        &original,
        DraftVersion::try_new(1)?,
        &DraftCommand::Set {
            pointer: "/nested/title~1value".into(),
            value: serde_json::json!("After"),
        },
    )?;
    assert_eq!(changed.version.get(), 2);
    assert_eq!(changed.document["nested"]["title/value"], "After");
    assert_eq!(original.document["nested"]["title/value"], "Before");
    Ok(())
}

#[test]
fn stale_version_and_non_object_result_are_rejected() -> Result<(), Box<dyn Error>> {
    let original = draft()?;
    let stale = preview_draft_mutation(
        &original,
        DraftVersion::try_new(2)?,
        &DraftCommand::Remove {
            pointer: "/nested".into(),
        },
    )
    .err()
    .ok_or_else(|| std::io::Error::other("stale mutation unexpectedly succeeded"))?;
    assert_eq!(
        stale,
        DraftEditError::VersionConflict {
            expected: 2,
            current: 1
        }
    );

    let invalid = preview_draft_mutation(
        &original,
        DraftVersion::try_new(1)?,
        &DraftCommand::ReplaceDocument {
            document: serde_json::json!(["not", "an", "object"]),
        },
    )
    .err()
    .ok_or_else(|| std::io::Error::other("non-object workspace unexpectedly succeeded"))?;
    assert!(matches!(invalid, DraftEditError::InvalidCommand(_)));
    Ok(())
}
