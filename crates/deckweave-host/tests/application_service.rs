// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{
    ApiAuthScope, ApiErrorCode, AppCommand, AppCommandResult, AppQuery, AppQueryResult,
    ApplicationGateway, ApplicationService, CommandClaimRepository, CommandEnvelope, DraftCommand,
    DraftStore, InMemoryApiAuthorizer, PrincipalId, RequestContext,
};
use deckweave_domain::{
    DeploymentMode,
    ids::{DedupEpoch, DraftId, DraftVersion, IdempotencyKey, TraceId, WorkspaceId},
};
use deckweave_host::HostRuntime;
use deckweave_platform::{HostLease, HostPlatform, PlatformError};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{JsonProfileStore, ProfileStoreLimits, WorkspaceDraft};
use std::{collections::BTreeMap, error::Error, path::PathBuf, sync::Arc};

struct TestLease;
impl HostLease for TestLease {}

struct TestPlatform {
    root: PathBuf,
}

impl HostPlatform for TestPlatform {
    fn data_root(&self) -> &std::path::Path {
        &self.root
    }

    fn mode(&self) -> DeploymentMode {
        DeploymentMode::Installed
    }

    fn acquire(&self) -> Result<Box<dyn HostLease>, PlatformError> {
        Ok(Box::new(TestLease))
    }

    fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        Ok(false)
    }

    fn set_autostart(&self, _enabled: bool) -> Result<(), PlatformError> {
        Ok(())
    }
}

fn draft() -> Result<WorkspaceDraft, Box<dyn Error>> {
    Ok(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("draft-api")?,
        workspace_id: WorkspaceId::try_new("workspace-api")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({"title": "Before"}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
}

fn context(principal: &str, trace: &str) -> Result<RequestContext, Box<dyn Error>> {
    Ok(RequestContext {
        principal_id: PrincipalId::try_new(principal)?,
        trace_id: TraceId::try_new(trace)?,
    })
}

fn envelope(
    epoch: &DedupEpoch,
    key: &str,
    value: &str,
    dry_run: bool,
) -> Result<CommandEnvelope, Box<dyn Error>> {
    Ok(CommandEnvelope {
        request_id: format!("request-{key}"),
        idempotency_key: IdempotencyKey::try_new(key)?,
        dedup_epoch: epoch.clone(),
        dry_run,
        command: AppCommand::PatchDraft {
            draft_id: DraftId::try_new("draft-api")?,
            expected_version: DraftVersion::try_new(1)?,
            edit: DraftCommand::Set {
                pointer: "/title".into(),
                value: serde_json::json!(value),
            },
        },
    })
}

fn service(
    root: PathBuf,
    authorizer: Arc<InMemoryApiAuthorizer>,
) -> Result<ApplicationGateway, Box<dyn Error>> {
    let store = Arc::new(JsonProfileStore::new(
        root.join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    let host = Arc::new(HostRuntime::start(Arc::new(TestPlatform { root }))?);
    Ok(ApplicationGateway::new(
        host,
        Arc::clone(&store) as Arc<dyn DraftStore>,
        store as Arc<dyn CommandClaimRepository>,
        authorizer,
        Arc::new(deckweave_application::UnavailableApplyUseCase),
    ))
}

#[test]
fn replay_returns_the_document_committed_with_its_receipt() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let setup_store = JsonProfileStore::new(
        directory.path().join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    );
    let original = draft()?;
    setup_store.create_draft(&original)?;
    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    let caller = context("editor", "replay")?;
    authorizer.grant(
        caller.principal_id.clone(),
        [ApiAuthScope::Read, ApiAuthScope::Draft],
    );
    let gateway = service(directory.path().to_path_buf(), Arc::clone(&authorizer))?;
    let epoch = gateway.open_mutation_window(&caller)?.dedup_epoch;
    let first = envelope(&epoch, "first", "First", false)?;
    gateway.command(&caller, first.clone())?;
    let mut second = envelope(&epoch, "second", "Second", false)?;
    let AppCommand::PatchDraft {
        expected_version, ..
    } = &mut second.command
    else {
        return Err(std::io::Error::other("expected draft command").into());
    };
    *expected_version = DraftVersion::try_new(2)?;
    gateway.command(&caller, second)?;
    drop(gateway);
    let reopened = service(directory.path().to_path_buf(), authorizer)?;
    let AppCommandResult::DraftMutation {
        draft,
        receipt: Some(receipt),
        ..
    } = reopened.command(&caller, first)?
    else {
        return Err(std::io::Error::other("missing receipt").into());
    };
    assert!(receipt.replayed);
    assert_eq!(draft.version, receipt.after_version);
    assert_eq!(draft.document["title"], "First");
    let AppQueryResult::Draft(current) = reopened.query(
        &caller,
        AppQuery::Draft {
            draft_id: original.draft_id,
        },
    )?
    else {
        return Err(std::io::Error::other("missing current draft").into());
    };
    assert_eq!(current.version.get(), 3);
    assert_eq!(current.document["title"], "Second");
    Ok(())
}

#[test]
fn in_process_client_enforces_scopes_dry_run_conflicts_and_dedup() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let profile_root = directory.path().join("profiles");
    let setup_store = JsonProfileStore::new(
        profile_root,
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    );
    let original = draft()?;
    setup_store.create_draft(&original)?;

    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    let allowed = context("allowed-client", "trace-allowed")?;
    authorizer.grant(
        allowed.principal_id.clone(),
        [ApiAuthScope::Read, ApiAuthScope::Draft],
    );
    let host = service(directory.path().to_path_buf(), Arc::clone(&authorizer))?;
    let epoch = host.open_mutation_window(&allowed)?.dedup_epoch;

    let dry_run = host.command(&allowed, envelope(&epoch, "mutation-1", "Preview", true)?)?;
    let AppCommandResult::DraftMutation {
        draft,
        receipt,
        dry_run,
    } = dry_run
    else {
        return Err(std::io::Error::other("unexpected dry-run result").into());
    };
    assert!(dry_run);
    assert!(receipt.is_none());
    assert_eq!(draft.version.get(), 2);
    assert_eq!(draft.document["title"], "Preview");
    let AppQueryResult::Draft(stored) = host.query(
        &allowed,
        AppQuery::Draft {
            draft_id: original.draft_id.clone(),
        },
    )?
    else {
        return Err(std::io::Error::other("unexpected query result").into());
    };
    assert_eq!(stored.version.get(), 1);
    assert_eq!(stored.document["title"], "Before");

    let mutation = envelope(&epoch, "mutation-1", "After", false)?;
    let first = host.command(&allowed, mutation.clone())?;
    let AppCommandResult::DraftMutation {
        receipt: Some(first_receipt),
        draft,
        ..
    } = first
    else {
        return Err(std::io::Error::other("missing mutation receipt").into());
    };
    assert!(!first_receipt.replayed);
    assert_eq!(draft.version.get(), 2);

    let replay = host.command(&allowed, mutation.clone())?;
    let AppCommandResult::DraftMutation {
        receipt: Some(replay_receipt),
        ..
    } = replay
    else {
        return Err(std::io::Error::other("missing replay receipt").into());
    };
    assert!(replay_receipt.replayed);
    assert_eq!(
        replay_receipt.document_sha256,
        first_receipt.document_sha256
    );

    let conflict = host
        .command(&allowed, envelope(&epoch, "mutation-2", "Stale", false)?)
        .err()
        .ok_or_else(|| std::io::Error::other("stale mutation unexpectedly succeeded"))?;
    assert_eq!(conflict.code, ApiErrorCode::RevisionConflict);
    assert_eq!(conflict.details["current"], 2);

    let reused_key = host
        .command(
            &allowed,
            envelope(&epoch, "mutation-1", "Different", false)?,
        )
        .err()
        .ok_or_else(|| std::io::Error::other("reused key unexpectedly succeeded"))?;
    assert_eq!(reused_key.code, ApiErrorCode::IdempotencyConflict);

    let denied = context("readless-client", "trace-denied")?;
    let denied_error = host
        .query(&denied, AppQuery::Host)
        .err()
        .ok_or_else(|| std::io::Error::other("scope-less query unexpectedly succeeded"))?;
    assert_eq!(denied_error.code, ApiErrorCode::PermissionDenied);
    assert_eq!(denied_error.trace_id, denied.trace_id);

    authorizer.revoke(&allowed.principal_id);
    let revoked_error = host
        .command(&allowed, mutation.clone())
        .err()
        .ok_or_else(|| std::io::Error::other("revoked mutation unexpectedly succeeded"))?;
    assert_eq!(revoked_error.code, ApiErrorCode::PermissionDenied);

    authorizer.grant(
        allowed.principal_id.clone(),
        [ApiAuthScope::Read, ApiAuthScope::Draft],
    );
    drop(host);
    let restarted = service(directory.path().to_path_buf(), authorizer)?;
    let replay_after_restart = restarted.command(&allowed, mutation)?;
    let AppCommandResult::DraftMutation {
        receipt: Some(receipt),
        ..
    } = replay_after_restart
    else {
        return Err(std::io::Error::other("missing restarted receipt").into());
    };
    assert!(receipt.replayed);
    Ok(())
}
