// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{
    ApiAuthScope, ApiErrorCode, AppCommand, AppQuery, ApplicationGateway, ApplicationService,
    CommandClaim, CommandClaimOutcome, CommandClaimRepository, CommandEnvelope, DraftCommand,
    DraftStore, DraftStoreError, DraftView, InMemoryApiAuthorizer, MutationWindow, PrincipalId,
    RequestContext, StoredDraftMutation, WorkspaceCandidate,
};
use deckweave_domain::{
    DeploymentMode, LifecycleState,
    ids::{DedupEpoch, DraftId, DraftVersion, IdempotencyKey, TraceId},
};
use deckweave_host::HostRuntime;
use deckweave_platform::{HostLease, HostPlatform, PlatformError};
use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct TestLease;
impl HostLease for TestLease {}

struct TestPlatform {
    root: PathBuf,
}

impl HostPlatform for TestPlatform {
    fn data_root(&self) -> &Path {
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

struct CharacterizationStore {
    error: DraftStoreError,
    calls: AtomicUsize,
}

impl CharacterizationStore {
    fn new(error: DraftStoreError) -> Self {
        Self {
            error,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn fail<T>(&self) -> Result<T, DraftStoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(self.error.clone())
    }
}

impl DraftStore for CharacterizationStore {
    fn open_mutation_window(
        &self,
        _principal_id: &PrincipalId,
    ) -> Result<MutationWindow, DraftStoreError> {
        self.fail()
    }

    fn get_draft(&self, _draft_id: &DraftId) -> Result<DraftView, DraftStoreError> {
        self.fail()
    }

    fn get_candidate(&self, _draft_id: &DraftId) -> Result<WorkspaceCandidate, DraftStoreError> {
        self.fail()
    }

    fn preview_draft_mutation(
        &self,
        _draft_id: &DraftId,
        _expected_version: DraftVersion,
        _command: &DraftCommand,
    ) -> Result<DraftView, DraftStoreError> {
        self.fail()
    }

    fn mutate_draft(
        &self,
        _draft_id: &DraftId,
        _key: deckweave_application::MutationKey,
        _expected_version: DraftVersion,
        _command: &DraftCommand,
    ) -> Result<StoredDraftMutation, DraftStoreError> {
        self.fail()
    }
}

impl CommandClaimRepository for CharacterizationStore {
    fn claim_command(&self, _claim: &CommandClaim) -> Result<CommandClaimOutcome, DraftStoreError> {
        self.fail()
    }
}

fn context(principal: &str, trace: &str) -> Result<RequestContext, Box<dyn Error>> {
    Ok(RequestContext {
        principal_id: deckweave_application::PrincipalId::try_new(principal)?,
        trace_id: TraceId::try_new(trace)?,
    })
}

fn service(
    root: PathBuf,
    store: Arc<CharacterizationStore>,
    authorizer: Arc<InMemoryApiAuthorizer>,
) -> Result<ApplicationGateway, PlatformError> {
    let host = Arc::new(HostRuntime::start(Arc::new(TestPlatform { root }))?);
    Ok(ApplicationGateway::new(
        host,
        Arc::clone(&store) as Arc<dyn DraftStore>,
        store as Arc<dyn CommandClaimRepository>,
        authorizer,
        Arc::new(deckweave_application::UnavailableApplyUseCase),
    ))
}

fn invalid_envelope() -> Result<CommandEnvelope, Box<dyn Error>> {
    Ok(CommandEnvelope {
        request_id: String::new(),
        idempotency_key: IdempotencyKey::try_new("invalid-request")?,
        dedup_epoch: DedupEpoch::try_new("epoch-1")?,
        dry_run: false,
        command: AppCommand::PatchDraft {
            draft_id: DraftId::try_new("draft-characterization")?,
            expected_version: DraftVersion::try_new(1)?,
            edit: DraftCommand::Set {
                pointer: "/title".into(),
                value: serde_json::json!("After"),
            },
        },
    })
}

#[test]
fn snapshot_preserves_lifecycle_notice_mode_and_root_without_services() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let root = directory.path().to_path_buf();
    let host = HostRuntime::start(Arc::new(TestPlatform { root: root.clone() }))?;

    let started = host.snapshot();
    assert_eq!(started.lifecycle, LifecycleState::Running);
    assert_eq!(started.mode, DeploymentMode::Installed);
    assert_eq!(started.data_root, root);
    assert_eq!(started.notice, None);

    host.set_notice(Some("characterized notice".into()));
    host.stop();
    let stopping = host.snapshot();
    assert_eq!(stopping.lifecycle, LifecycleState::Stopping);
    assert_eq!(stopping.notice.as_deref(), Some("characterized notice"));
    Ok(())
}

#[test]
fn authorization_and_request_validation_run_before_store_dispatch() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = Arc::new(CharacterizationStore::new(DraftStoreError::NotFound));
    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    let host = service(
        directory.path().to_path_buf(),
        Arc::clone(&store),
        Arc::clone(&authorizer),
    )?;
    let principal = context("ordered-client", "trace-order")?;

    let unauthorized = host
        .command(&principal, invalid_envelope()?)
        .err()
        .ok_or_else(|| std::io::Error::other("authorization unexpectedly allowed the command"))?;
    assert_eq!(unauthorized.code, ApiErrorCode::PermissionDenied);
    assert_eq!(store.calls(), 0);

    authorizer.grant(principal.principal_id.clone(), [ApiAuthScope::Draft]);
    let invalid = host
        .command(&principal, invalid_envelope()?)
        .err()
        .ok_or_else(|| std::io::Error::other("empty request id unexpectedly reached the store"))?;
    assert_eq!(invalid.code, ApiErrorCode::InvalidCommand);
    assert_eq!(invalid.message_key, "api.error.invalid_request_id");
    assert_eq!(invalid.trace_id, principal.trace_id);
    assert_eq!(store.calls(), 0);
    Ok(())
}

#[test]
fn store_errors_keep_the_stable_application_error_contract() -> Result<(), Box<dyn Error>> {
    struct Case {
        store_error: DraftStoreError,
        code: ApiErrorCode,
        message_key: &'static str,
        retryable: bool,
        details: BTreeMap<String, serde_json::Value>,
    }

    let cases = [
        Case {
            store_error: DraftStoreError::NotFound,
            code: ApiErrorCode::NotFound,
            message_key: "api.error.draft_not_found",
            retryable: false,
            details: BTreeMap::new(),
        },
        Case {
            store_error: DraftStoreError::VersionConflict {
                expected: 4,
                current: 7,
            },
            code: ApiErrorCode::RevisionConflict,
            message_key: "api.error.revision_conflict",
            retryable: false,
            details: BTreeMap::from([
                ("expected".into(), serde_json::json!(4)),
                ("current".into(), serde_json::json!(7)),
            ]),
        },
        Case {
            store_error: DraftStoreError::IdempotencyConflict,
            code: ApiErrorCode::IdempotencyConflict,
            message_key: "api.error.idempotency_conflict",
            retryable: false,
            details: BTreeMap::new(),
        },
        Case {
            store_error: DraftStoreError::IdempotencyExpired,
            code: ApiErrorCode::IdempotencyExpired,
            message_key: "api.error.idempotency_expired",
            retryable: false,
            details: BTreeMap::new(),
        },
        Case {
            store_error: DraftStoreError::ReceiptQuota,
            code: ApiErrorCode::CapacityExceeded,
            message_key: "api.error.mutation_receipt_quota",
            retryable: false,
            details: BTreeMap::new(),
        },
        Case {
            store_error: DraftStoreError::InvalidCommand("bad pointer".into()),
            code: ApiErrorCode::InvalidCommand,
            message_key: "api.error.invalid_command",
            retryable: false,
            details: BTreeMap::from([("reason".into(), serde_json::json!("bad pointer"))]),
        },
        Case {
            store_error: DraftStoreError::InvalidCandidate("missing asset".into()),
            code: ApiErrorCode::InvalidCommand,
            message_key: "api.error.invalid_candidate",
            retryable: false,
            details: BTreeMap::from([("reason".into(), serde_json::json!("missing asset"))]),
        },
        Case {
            store_error: DraftStoreError::Unavailable("disk offline".into()),
            code: ApiErrorCode::Unavailable,
            message_key: "api.error.profile_store_unavailable",
            retryable: true,
            details: BTreeMap::from([("reason".into(), serde_json::json!("disk offline"))]),
        },
    ];

    for (index, case) in cases.into_iter().enumerate() {
        let directory = tempfile::tempdir()?;
        let authorizer = Arc::new(InMemoryApiAuthorizer::new());
        let principal = context("error-client", &format!("trace-error-{index}"))?;
        authorizer.grant(principal.principal_id.clone(), [ApiAuthScope::Read]);
        let host = service(
            directory.path().to_path_buf(),
            Arc::new(CharacterizationStore::new(case.store_error)),
            authorizer,
        )?;

        let error = host
            .query(
                &principal,
                AppQuery::Draft {
                    draft_id: DraftId::try_new("draft-characterization")?,
                },
            )
            .err()
            .ok_or_else(|| {
                std::io::Error::other("characterization store unexpectedly returned a draft")
            })?;
        assert_eq!(error.code, case.code);
        assert_eq!(error.message_key, case.message_key);
        assert_eq!(error.retryable, case.retryable);
        assert_eq!(error.details, case.details);
        assert_eq!(error.trace_id, principal.trace_id);
    }
    Ok(())
}
