// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
#[test]
fn oversize_edit_is_a_capacity_error_and_preserves_the_saved_draft() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let store = JsonProfileStore::new(
        root.path().to_path_buf(),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits {
            maximum_revision_bytes: 4096,
            ..Default::default()
        },
    );
    let initial = draft()?;
    store.create_draft(&initial)?;
    let error = DraftRepository::mutate_draft(
        &store,
        &initial.draft_id,
        mutation_key(&store, "large")?,
        initial.version,
        &DraftCommand::Set {
            pointer: "/title".into(),
            value: serde_json::json!("x".repeat(5000)),
        },
    )
    .err()
    .ok_or("oversize edit accepted")?;
    assert_eq!(error, DraftRepositoryError::DocumentTooLarge);
    assert_eq!(
        DraftRepository::get_draft(&store, &initial.draft_id)?.document,
        initial.document
    );
    assert_eq!(
        DraftRepository::get_draft(&store, &initial.draft_id)?.version,
        initial.version
    );
    Ok(())
}
