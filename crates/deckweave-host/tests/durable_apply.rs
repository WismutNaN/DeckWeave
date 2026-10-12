// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{
    ApiAuthScope, AppCommand, AppCommandResult, AppQuery, AppQueryResult, ApplicationGateway,
    ApplicationService, CommandEnvelope, HostSnapshot, HostStatusSource, InMemoryApiAuthorizer,
    RequestContext,
};
use deckweave_automation::{RestrictedFixtureCompiler, VIRTUAL_FIXTURE_PACKAGE_ID};
use deckweave_domain::{
    DeploymentMode, LifecycleState,
    ids::{
        DraftId, DraftVersion, GrantRevision, HostBootId, IdempotencyKey, OperationId, PackageId,
        PrincipalId, RegistryRevision, TraceId, WorkspaceId,
    },
};
use deckweave_host::{
    ApplyCoordinator, ApplyPlan, ApplyPlanningError, ApplyPlanningService, PlanningClock,
    PlanningEvidence, PlanningEvidenceError, PlanningEvidenceSource, PreparedActivation,
    RuntimeActivation, RuntimeActivationError, RuntimeCommitOutcome, RuntimeCutover,
    RuntimeCutoverError,
};
use deckweave_platform::{AtomicFileStore, PublicationError, StorageError};
use deckweave_platform_host::NativeAtomicFileStore;
use deckweave_profiles::{JsonProfileStore, ProfileStoreLimits};
use deckweave_workspace::{
    ActiveSelection, ApplyCommitRecord, ApplyCommitStatus, ApplyOperationRepository,
    ApplyOperationState, DraftRepository, PackageLockEntry, WorkspaceDraft, WorkspaceRevision,
};
use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

struct Status;

impl HostStatusSource for Status {
    fn snapshot(&self) -> HostSnapshot {
        HostSnapshot {
            lifecycle: LifecycleState::Running,
            mode: DeploymentMode::Portable,
            data_root: PathBuf::from("test"),
            version: "test".into(),
            notice: None,
        }
    }
}

struct Evidence {
    value: Mutex<PlanningEvidence>,
}

impl Evidence {
    fn bump_grants(&self) -> Result<(), PlanningEvidenceError> {
        let mut evidence = self
            .value
            .lock()
            .map_err(|_| PlanningEvidenceError::Unavailable("evidence lock poisoned".into()))?;
        evidence.grant_revision = GrantRevision::try_new(evidence.grant_revision.get() + 1)
            .map_err(|error| PlanningEvidenceError::Unavailable(error.to_string()))?;
        Ok(())
    }
}

impl PlanningEvidenceSource for Evidence {
    fn snapshot(&self) -> Result<PlanningEvidence, PlanningEvidenceError> {
        self.value
            .lock()
            .map_err(|_| PlanningEvidenceError::Unavailable("evidence lock poisoned".into()))
            .map(|value| value.clone())
    }
}

struct Clock(AtomicU64);

impl PlanningClock for Clock {
    fn monotonic_millis(&self) -> Result<u64, ApplyPlanningError> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

struct DiskFullRevisionStore {
    enabled: AtomicBool,
}

impl DiskFullRevisionStore {
    fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
        }
    }
}

impl AtomicFileStore for DiskFullRevisionStore {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), PublicationError> {
        if self.enabled.load(Ordering::SeqCst)
            && target
                .components()
                .any(|part| part.as_os_str() == "revisions")
        {
            return Err(PublicationError::not_published(StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::StorageFull, "injected disk full"),
            )));
        }
        NativeAtomicFileStore.write_atomic(target, bytes, backup)
    }
}

struct Runtime {
    evidence: Arc<Evidence>,
    change_grants_at_barrier: AtomicBool,
    fail_disruptive_handoff: AtomicBool,
    dispatcher_closed: AtomicBool,
    activations: AtomicUsize,
    recoveries: AtomicUsize,
}

impl Runtime {
    fn new(evidence: Arc<Evidence>) -> Self {
        Self {
            evidence,
            change_grants_at_barrier: AtomicBool::new(false),
            fail_disruptive_handoff: AtomicBool::new(false),
            dispatcher_closed: AtomicBool::new(false),
            activations: AtomicUsize::new(0),
            recoveries: AtomicUsize::new(0),
        }
    }
}

impl RuntimeActivation for Runtime {
    fn prepare(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
    ) -> Result<PreparedActivation, RuntimeActivationError> {
        Ok(PreparedActivation::new(
            operation_id.clone(),
            plan.token().plan_hash().into(),
        ))
    }
}

impl RuntimeCutover for Runtime {
    fn enter_barrier(
        &self,
        _operation_id: &OperationId,
        plan: &ApplyPlan,
        _prepared: &PreparedActivation,
    ) -> Result<(), RuntimeCutoverError> {
        self.dispatcher_closed.store(true, Ordering::SeqCst);
        if plan.compiled().requires_disruption()
            && self.fail_disruptive_handoff.load(Ordering::SeqCst)
        {
            return Err(RuntimeCutoverError::Handoff(
                "injected exclusive resource transfer failure".into(),
            ));
        }
        if self.change_grants_at_barrier.load(Ordering::SeqCst) {
            self.evidence
                .bump_grants()
                .map_err(|error| RuntimeCutoverError::Barrier(error.to_string()))?;
        }
        Ok(())
    }

    fn abort_before_commit(&self, _operation_id: &OperationId) -> Result<(), RuntimeCutoverError> {
        self.dispatcher_closed.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn activate_committed(
        &self,
        _operation_id: &OperationId,
        _plan: &ApplyPlan,
        _prepared: &PreparedActivation,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError> {
        if !self.dispatcher_closed.swap(false, Ordering::SeqCst) {
            return Err(RuntimeCutoverError::Activation(
                "dispatcher was not closed".into(),
            ));
        }
        self.activations.fetch_add(1, Ordering::SeqCst);
        Ok(RuntimeCommitOutcome::Applied)
    }

    fn recover_committed(
        &self,
        _commit: &ApplyCommitRecord,
        _revision: &WorkspaceRevision,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError> {
        self.recoveries.fetch_add(1, Ordering::SeqCst);
        Ok(RuntimeCommitOutcome::Applied)
    }
}

fn draft() -> Result<WorkspaceDraft, Box<dyn Error>> {
    Ok(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("apply-draft")?,
        workspace_id: WorkspaceId::try_new("apply-workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
}

fn disruptive_draft() -> Result<WorkspaceDraft, Box<dyn Error>> {
    let mut value = draft()?;
    value.document = serde_json::json!({
        "fixture": {"kind": "virtual_disruptive_echo"}
    });
    value.package_lock.push(PackageLockEntry {
        package_id: PackageId::try_new(VIRTUAL_FIXTURE_PACKAGE_ID)?,
        version: "0.1.0".into(),
        artifact_sha256: "a".repeat(64),
        source: None,
        extensions: BTreeMap::new(),
    });
    Ok(value)
}

fn coordinator(
    repository: Arc<JsonProfileStore>,
    evidence: Arc<Evidence>,
    runtime: Arc<Runtime>,
) -> Result<Arc<ApplyCoordinator>, Box<dyn Error>> {
    let planning = ApplyPlanningService::new(
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        Arc::new(RestrictedFixtureCompiler),
        evidence as Arc<dyn PlanningEvidenceSource>,
        Arc::clone(&runtime) as Arc<dyn RuntimeActivation>,
        Arc::new(Clock(AtomicU64::new(100))),
        1_000,
    )?;
    Ok(Arc::new(ApplyCoordinator::new(
        planning,
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        repository as Arc<dyn ApplyOperationRepository>,
        runtime as Arc<dyn RuntimeCutover>,
        16,
    )?))
}

type Fixture = (
    ApplicationGateway,
    Arc<JsonProfileStore>,
    Arc<Evidence>,
    Arc<Runtime>,
    RequestContext,
);

fn fixture(root: PathBuf, active: Option<ActiveSelection>) -> Result<Fixture, Box<dyn Error>> {
    fixture_with_draft(root, active, draft()?)
}

fn fixture_with_draft(
    root: PathBuf,
    active: Option<ActiveSelection>,
    initial_draft: WorkspaceDraft,
) -> Result<Fixture, Box<dyn Error>> {
    fixture_with_store(root, active, initial_draft, Arc::new(NativeAtomicFileStore))
}

fn fixture_with_store(
    root: PathBuf,
    active: Option<ActiveSelection>,
    initial_draft: WorkspaceDraft,
    files: Arc<dyn AtomicFileStore>,
) -> Result<Fixture, Box<dyn Error>> {
    let repository = Arc::new(JsonProfileStore::new(
        root,
        files,
        ProfileStoreLimits::default(),
    ));
    if repository
        .get_draft(&DraftId::try_new("apply-draft")?)
        .is_err()
    {
        repository.create_draft(&initial_draft)?;
    }
    let evidence = Arc::new(Evidence {
        value: Mutex::new(PlanningEvidence {
            host_boot_id: HostBootId::try_new("boot-test")?,
            registry_revision: RegistryRevision::try_new(1)?,
            grant_revision: GrantRevision::try_new(1)?,
            active,
        }),
    });
    let runtime = Arc::new(Runtime::new(Arc::clone(&evidence)));
    let apply = coordinator(
        Arc::clone(&repository),
        Arc::clone(&evidence),
        Arc::clone(&runtime),
    )?;
    let principal = PrincipalId::try_new("desktop-user")?;
    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    authorizer.grant(
        principal.clone(),
        [
            ApiAuthScope::Read,
            ApiAuthScope::Draft,
            ApiAuthScope::Simulation,
            ApiAuthScope::Apply,
        ],
    );
    let gateway = ApplicationGateway::new(
        Arc::new(Status),
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        authorizer,
        apply,
    );
    Ok((
        gateway,
        repository,
        evidence,
        runtime,
        RequestContext {
            principal_id: principal,
            trace_id: TraceId::try_new("trace-apply")?,
        },
    ))
}

fn plan_hash(
    gateway: &ApplicationGateway,
    context: &RequestContext,
) -> Result<String, Box<dyn Error>> {
    plan_hash_with_policy(gateway, context, false)
}

fn plan_hash_with_policy(
    gateway: &ApplicationGateway,
    context: &RequestContext,
    allow_disruption: bool,
) -> Result<String, Box<dyn Error>> {
    match gateway.query(
        context,
        AppQuery::PlanApply {
            draft_id: DraftId::try_new("apply-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
            allow_disruption,
        },
    )? {
        AppQueryResult::ApplyPlan(plan) => Ok(plan.plan_hash),
        _ => Err(std::io::Error::other("unexpected plan response").into()),
    }
}

#[test]
fn public_gateway_applies_queries_and_replays_one_durable_operation() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let (gateway, repository, _, runtime, context) = fixture(directory.path().to_path_buf(), None)?;
    let window = gateway.open_mutation_window(&context)?;
    let plan_hash = plan_hash(&gateway, &context)?;
    let envelope = CommandEnvelope {
        request_id: "request-apply".into(),
        idempotency_key: IdempotencyKey::try_new("apply-key")?,
        dedup_epoch: window.dedup_epoch,
        dry_run: false,
        command: AppCommand::ApplyDraft {
            draft_id: DraftId::try_new("apply-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
            allow_disruption: false,
            plan_hash,
        },
    };
    let operation_id = match gateway.command(&context, envelope.clone())? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err(std::io::Error::other("unexpected apply response").into()),
    };
    let replayed_id = match gateway.command(&context, envelope)? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err(std::io::Error::other("unexpected replay response").into()),
    };
    assert_eq!(operation_id, replayed_id);
    assert_eq!(runtime.activations.load(Ordering::SeqCst), 1);
    let snapshot = match gateway.query(&context, AppQuery::Operation { operation_id })? {
        AppQueryResult::Operation(snapshot) => snapshot,
        _ => return Err(std::io::Error::other("unexpected operation response").into()),
    };
    assert_eq!(snapshot.state, ApplyOperationState::Succeeded);
    assert_eq!(snapshot.commit_status, ApplyCommitStatus::Committed);
    assert_eq!(
        repository.active_commit()?.map(|commit| commit.candidate),
        Some(snapshot.intent.candidate)
    );
    Ok(())
}

#[test]
fn grant_change_at_barrier_aborts_without_switching_selection() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (gateway, repository, _, runtime, context) = fixture(directory.path().to_path_buf(), None)?;
    runtime
        .change_grants_at_barrier
        .store(true, Ordering::SeqCst);
    let window = gateway.open_mutation_window(&context)?;
    let result = gateway.command(
        &context,
        CommandEnvelope {
            request_id: "request-revoke".into(),
            idempotency_key: IdempotencyKey::try_new("revoke-key")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("apply-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: false,
                plan_hash: plan_hash(&gateway, &context)?,
            },
        },
    )?;
    let operation_id = match result {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err(std::io::Error::other("unexpected apply response").into()),
    };
    assert!(repository.active_commit()?.is_none());
    assert!(!runtime.dispatcher_closed.load(Ordering::SeqCst));
    assert_eq!(
        repository.operation(&operation_id)?.state,
        ApplyOperationState::Failed
    );
    Ok(())
}

#[test]
fn grant_revoke_before_prepare_fails_without_switching_selection() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (gateway, repository, evidence, runtime, context) =
        fixture(directory.path().to_path_buf(), None)?;
    let plan_hash = plan_hash(&gateway, &context)?;
    evidence.bump_grants()?;
    let window = gateway.open_mutation_window(&context)?;
    let operation_id = match gateway.command(
        &context,
        CommandEnvelope {
            request_id: "request-revoke-before-prepare".into(),
            idempotency_key: IdempotencyKey::try_new("revoke-before-prepare")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("apply-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: false,
                plan_hash,
            },
        },
    )? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err(std::io::Error::other("unexpected apply response").into()),
    };
    assert!(repository.active_commit()?.is_none());
    assert_eq!(
        repository.operation(&operation_id)?.state,
        ApplyOperationState::Failed
    );
    assert_eq!(runtime.activations.load(Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn disk_full_while_saving_candidate_leaves_no_intent_or_commit() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let files = Arc::new(DiskFullRevisionStore::new());
    let (gateway, repository, _, runtime, context) = fixture_with_store(
        directory.path().to_path_buf(),
        None,
        draft()?,
        Arc::clone(&files) as Arc<dyn AtomicFileStore>,
    )?;
    let plan_hash = plan_hash(&gateway, &context)?;
    let window = gateway.open_mutation_window(&context)?;
    files.enabled.store(true, Ordering::SeqCst);

    let result = gateway.command(
        &context,
        CommandEnvelope {
            request_id: "request-disk-full".into(),
            idempotency_key: IdempotencyKey::try_new("disk-full")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("apply-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: false,
                plan_hash,
            },
        },
    );
    assert!(result.is_err());
    assert!(repository.active_commit()?.is_none());
    assert!(repository.recoverable_operations(16)?.is_empty());
    assert_eq!(runtime.activations.load(Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn failed_exclusive_handoff_reopens_dispatch_without_committing() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (gateway, repository, _, runtime, context) =
        fixture_with_draft(directory.path().to_path_buf(), None, disruptive_draft()?)?;
    assert!(
        gateway
            .query(
                &context,
                AppQuery::PlanApply {
                    draft_id: DraftId::try_new("apply-draft")?,
                    expected_draft_version: DraftVersion::try_new(1)?,
                    expected_active: None,
                    allow_disruption: false,
                },
            )
            .is_err()
    );
    let plan_hash = plan_hash_with_policy(&gateway, &context, true)?;
    runtime
        .fail_disruptive_handoff
        .store(true, Ordering::SeqCst);
    let window = gateway.open_mutation_window(&context)?;
    let operation_id = match gateway.command(
        &context,
        CommandEnvelope {
            request_id: "request-handoff-failure".into(),
            idempotency_key: IdempotencyKey::try_new("handoff-failure")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("apply-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: true,
                plan_hash,
            },
        },
    )? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err(std::io::Error::other("unexpected apply response").into()),
    };

    let snapshot = repository.operation(&operation_id)?;
    assert!(snapshot.intent.allow_disruption);
    assert_eq!(snapshot.state, ApplyOperationState::Failed);
    assert!(repository.active_commit()?.is_none());
    assert!(!runtime.dispatcher_closed.load(Ordering::SeqCst));
    assert_eq!(runtime.activations.load(Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn startup_reactivates_the_committed_revision_without_replaying_an_input()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (gateway, repository, _, _, context) = fixture(directory.path().to_path_buf(), None)?;
    let window = gateway.open_mutation_window(&context)?;
    let result = gateway.command(
        &context,
        CommandEnvelope {
            request_id: "request-restart".into(),
            idempotency_key: IdempotencyKey::try_new("restart-key")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("apply-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: false,
                plan_hash: plan_hash(&gateway, &context)?,
            },
        },
    )?;
    let operation_id = match result {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err(std::io::Error::other("unexpected apply response").into()),
    };
    let active = repository
        .active_commit()?
        .ok_or_else(|| std::io::Error::other("missing active commit"))?;
    drop(gateway);
    drop(repository);

    let reopened = Arc::new(JsonProfileStore::new(
        directory.path().to_path_buf(),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    let evidence = Arc::new(Evidence {
        value: Mutex::new(PlanningEvidence {
            host_boot_id: HostBootId::try_new("boot-restarted")?,
            registry_revision: RegistryRevision::try_new(1)?,
            grant_revision: GrantRevision::try_new(1)?,
            active: Some(active.candidate.clone()),
        }),
    });
    let runtime = Arc::new(Runtime::new(Arc::clone(&evidence)));
    let coordinator = coordinator(Arc::clone(&reopened), evidence, Arc::clone(&runtime))?;
    let recovered = coordinator.recover(16)?;
    assert_eq!(runtime.recoveries.load(Ordering::SeqCst), 1);
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].intent.operation_id, operation_id);
    assert_eq!(recovered[0].state, ApplyOperationState::Succeeded);
    Ok(())
}
