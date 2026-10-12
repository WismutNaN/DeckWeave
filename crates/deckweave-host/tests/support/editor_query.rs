// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::editor::{EditorReadSource, EditorSnapshot, EditorWorkspaceService};
struct ReadSource(AtomicUsize);
impl EditorReadSource for ReadSource {
    fn editor_snapshot(&self) -> Result<EditorSnapshot, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(EditorSnapshot {
            registry_revision: 7,
            installed: vec![],
            nodes: vec![],
            runtimes: vec![],
            active: None,
        })
    }
}
#[test]
fn editor_reads_and_open_authorize_before_io_and_preserve_error_dto() -> Result<(), Box<dyn Error>>
{
    let root = tempfile::tempdir()?;
    let store = Arc::new(CharacterizationStore::new(DraftStoreError::Unavailable(
        "test store unavailable".into(),
    )));
    let auth = Arc::new(InMemoryApiAuthorizer::new());
    let read = Arc::new(ReadSource(AtomicUsize::new(0)));
    let gateway = service(root.path().to_owned(), store.clone(), auth.clone())?
        .with_editor_source(read.clone());
    let context = context("editor-client", "editor-trace")?;
    let denied = gateway
        .query(&context, AppQuery::EditorSnapshot)
        .err()
        .ok_or("denied")?;
    assert_eq!(denied.code, ApiErrorCode::PermissionDenied);
    assert_eq!(read.0.load(Ordering::SeqCst), 0);
    auth.grant(context.principal_id.clone(), [ApiAuthScope::Read]);
    let dto = serde_json::to_value(gateway.query(&context, AppQuery::EditorSnapshot)?)?;
    assert_eq!(dto["type"], "editor_snapshot");
    assert_eq!(dto["registryRevision"], 7);
    assert!(dto["active"].is_null());
    let initial = deckweave_workspace::WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("editor")?,
        workspace_id: deckweave_domain::ids::WorkspaceId::try_new("workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({}),
        assets: vec![],
        private_section: None,
        extensions: Default::default(),
        unknown_fields: Default::default(),
    };
    let editor = EditorWorkspaceService::new(store.clone(), auth.clone(), initial);
    assert_eq!(
        editor.open(&context).err().ok_or("denied")?.code,
        ApiErrorCode::PermissionDenied
    );
    assert_eq!(store.calls(), 0);
    auth.grant(context.principal_id.clone(), [ApiAuthScope::Draft]);
    let error = editor.open(&context).err().ok_or("unavailable")?;
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert!(error.retryable);
    assert_eq!(error.trace_id, context.trace_id);
    assert_eq!(store.calls(), 1);
    Ok(())
}
