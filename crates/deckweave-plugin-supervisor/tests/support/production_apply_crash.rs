// SPDX-License-Identifier: GPL-3.0-or-later
//! Out-of-process crash matrix for the production apply composition.

use super::*;
use deckweave_host::{
    ApplyPlan, PreparedActivation, RuntimeCommitOutcome, RuntimeCutover, RuntimeCutoverError,
};
use deckweave_workspace::{
    ActiveSelection, ApplyBegin, ApplyCommitRecord, ApplyOperationIntent, ApplyOperationSnapshot,
    ApplyRepositoryError, ApplyRuntimeHealth, WorkspaceRevision,
};
use std::process::{Child, Command};

const CRASH_ROOT: &str = "DECKWEAVE_APPLY_CRASH_ROOT";
const CRASH_POINT: &str = "DECKWEAVE_APPLY_CRASH_POINT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrashPoint {
    Candidate,
    Intent,
    Prepared,
    Barrier,
    Commit,
    Activation,
}

impl CrashPoint {
    const ALL: [Self; 6] = [
        Self::Candidate,
        Self::Intent,
        Self::Prepared,
        Self::Barrier,
        Self::Commit,
        Self::Activation,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Intent => "intent",
            Self::Prepared => "prepared",
            Self::Barrier => "barrier",
            Self::Commit => "commit",
            Self::Activation => "activation",
        }
    }

    fn parse(value: &str) -> Result<Self, Box<dyn Error>> {
        Self::ALL
            .into_iter()
            .find(|point| point.as_str() == value)
            .ok_or_else(|| format!("unknown crash point {value}").into())
    }

    fn is_committed(self) -> bool {
        matches!(self, Self::Commit | Self::Activation)
    }
}

struct CrashRepository {
    inner: Arc<JsonProfileStore>,
    point: CrashPoint,
}

impl CrashRepository {
    fn trip(&self, point: CrashPoint) {
        if self.point == point {
            std::process::abort();
        }
    }
}

impl ApplyOperationRepository for CrashRepository {
    fn save_candidate_revision(
        &self,
        revision: &WorkspaceRevision,
    ) -> Result<(), ApplyRepositoryError> {
        self.inner.save_candidate_revision(revision)?;
        self.trip(CrashPoint::Candidate);
        Ok(())
    }

    fn load_candidate_revision(
        &self,
        selection: &ActiveSelection,
    ) -> Result<WorkspaceRevision, ApplyRepositoryError> {
        self.inner.load_candidate_revision(selection)
    }

    fn begin_apply(
        &self,
        intent: &ApplyOperationIntent,
    ) -> Result<ApplyBegin, ApplyRepositoryError> {
        let result = self.inner.begin_apply(intent)?;
        self.trip(CrashPoint::Intent);
        Ok(result)
    }

    fn mark_prepared(
        &self,
        operation_id: &deckweave_domain::ids::OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        let result = self.inner.mark_prepared(operation_id)?;
        self.trip(CrashPoint::Prepared);
        Ok(result)
    }

    fn publish_commit(
        &self,
        commit: &ApplyCommitRecord,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        let result = self.inner.publish_commit(commit)?;
        self.trip(CrashPoint::Commit);
        Ok(result)
    }

    fn finish_apply(
        &self,
        operation_id: &deckweave_domain::ids::OperationId,
        state: ApplyOperationState,
        runtime_health: ApplyRuntimeHealth,
        detail: Option<String>,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        self.inner
            .finish_apply(operation_id, state, runtime_health, detail)
    }

    fn operation(
        &self,
        operation_id: &deckweave_domain::ids::OperationId,
    ) -> Result<ApplyOperationSnapshot, ApplyRepositoryError> {
        self.inner.operation(operation_id)
    }

    fn active_commit(&self) -> Result<Option<ApplyCommitRecord>, ApplyRepositoryError> {
        self.inner.active_commit()
    }

    fn recoverable_operations(
        &self,
        maximum: usize,
    ) -> Result<Vec<ApplyOperationSnapshot>, ApplyRepositoryError> {
        self.inner.recoverable_operations(maximum)
    }
}

struct CrashCutover {
    inner: Arc<ProductionRuntimeCoordinator>,
    point: CrashPoint,
}

impl RuntimeCutover for CrashCutover {
    fn enter_barrier(
        &self,
        operation_id: &deckweave_domain::ids::OperationId,
        plan: &ApplyPlan,
        prepared: &PreparedActivation,
    ) -> Result<(), RuntimeCutoverError> {
        self.inner.enter_barrier(operation_id, plan, prepared)?;
        if self.point == CrashPoint::Barrier {
            std::process::abort();
        }
        Ok(())
    }

    fn abort_before_commit(
        &self,
        operation_id: &deckweave_domain::ids::OperationId,
    ) -> Result<(), RuntimeCutoverError> {
        self.inner.abort_before_commit(operation_id)
    }

    fn activate_committed(
        &self,
        operation_id: &deckweave_domain::ids::OperationId,
        plan: &ApplyPlan,
        prepared: &PreparedActivation,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError> {
        let result = self
            .inner
            .activate_committed(operation_id, plan, prepared)?;
        if self.point == CrashPoint::Activation {
            std::process::abort();
        }
        Ok(result)
    }

    fn recover_committed(
        &self,
        commit: &ApplyCommitRecord,
        revision: &WorkspaceRevision,
    ) -> Result<RuntimeCommitOutcome, RuntimeCutoverError> {
        self.inner.recover_committed(commit, revision)
    }
}

fn crash_apply(root: &Path, point: CrashPoint) -> Result<(), Box<dyn Error>> {
    let artifact_sha256 = stage_package(root)?;
    let repository = Arc::new(JsonProfileStore::new(
        root.join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    repository.create_draft(&draft(artifact_sha256)?)?;
    let authorization = Arc::new(MutableAuthorization::new());
    let packages = Arc::new(PackageIndex::scan(
        &root.join("packages"),
        &StagingLimits::default(),
    )?);
    let registry = Arc::new(Mutex::new(CapabilityRegistry::default()));
    let runtime = Arc::new(ProductionRuntimeCoordinator::new(
        packages,
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        Arc::clone(&repository) as Arc<dyn ApplyOperationRepository>,
        authorization as Arc<dyn AuthorizationStore>,
        registry,
        Arc::new(NativeChildProcessPolicy),
        PrincipalId::try_new("desktop-user")?,
        HostBootId::try_new("boot-crash")?,
        HostRuntimeConfig::default(),
    ));
    let planning = ApplyPlanningService::new(
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        Arc::new(RestrictedFixtureCompiler),
        Arc::clone(&runtime) as Arc<dyn deckweave_host::PlanningEvidenceSource>,
        Arc::clone(&runtime) as Arc<dyn deckweave_host::RuntimeActivation>,
        Arc::clone(&runtime) as Arc<dyn deckweave_host::PlanningClock>,
        30_000,
    )?;
    let operations: Arc<dyn ApplyOperationRepository> = Arc::new(CrashRepository {
        inner: Arc::clone(&repository),
        point,
    });
    let cutover: Arc<dyn RuntimeCutover> = Arc::new(CrashCutover {
        inner: runtime,
        point,
    });
    let apply = Arc::new(ApplyCoordinator::new(
        planning,
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        operations,
        cutover,
        16,
    )?);
    let (gateway, context) = gateway(root, repository, apply)?;
    let window = gateway.open_mutation_window(&context)?;
    let plan_hash = match gateway.query(
        &context,
        AppQuery::PlanApply {
            draft_id: DraftId::try_new("virtual-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
            allow_disruption: false,
        },
    )? {
        AppQueryResult::ApplyPlan(plan) => plan.plan_hash,
        _ => return Err("unexpected plan response".into()),
    };
    let _unexpected_success = gateway.command(
        &context,
        CommandEnvelope {
            request_id: format!("crash-{}", point.as_str()),
            idempotency_key: IdempotencyKey::try_new(format!("crash-{}", point.as_str()))?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("virtual-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: false,
                plan_hash,
            },
        },
    )?;
    Err(format!("crash point {} was not reached", point.as_str()).into())
}

#[test]
fn apply_crash_child() -> Result<(), Box<dyn Error>> {
    let Some(root) = std::env::var_os(CRASH_ROOT) else {
        return Ok(());
    };
    let point = CrashPoint::parse(&std::env::var(CRASH_POINT)?)?;
    crash_apply(Path::new(&root), point)
}

fn wait_for_crash(child: &mut Child) -> Result<std::process::ExitStatus, Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err("crash fixture exceeded watchdog".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn process_kill_at_each_apply_stage_recovers_the_durable_selection() -> Result<(), Box<dyn Error>> {
    for point in CrashPoint::ALL {
        let directory = tempfile::tempdir()?;
        let mut child = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "production_apply_crash::apply_crash_child",
                "--nocapture",
            ])
            .env(CRASH_ROOT, directory.path())
            .env(CRASH_POINT, point.as_str())
            .spawn()?;
        let status = wait_for_crash(&mut child)?;
        assert!(!status.success(), "{} did not crash", point.as_str());

        let repository = Arc::new(JsonProfileStore::new(
            directory.path().join("profiles"),
            Arc::new(NativeAtomicFileStore),
            ProfileStoreLimits::default(),
        ));
        let authorization = Arc::new(MutableAuthorization::new());
        let (runtime, apply, registry) = build_runtime(
            directory.path(),
            Arc::clone(&repository),
            authorization,
            &format!("boot-recover-{}", point.as_str()),
        )?;
        let recovered = apply.recover(16)?;
        if point.is_committed() {
            assert!(repository.active_commit()?.is_some(), "{}", point.as_str());
            assert_eq!(recovered.len(), 1, "{}", point.as_str());
            assert_eq!(recovered[0].state, ApplyOperationState::Succeeded);
            assert_eq!(
                registry
                    .lock()
                    .map_err(|_| "registry poisoned")?
                    .snapshot()?
                    .nodes
                    .len(),
                1,
                "{}",
                point.as_str()
            );
        } else {
            assert!(repository.active_commit()?.is_none(), "{}", point.as_str());
            let expected = usize::from(point != CrashPoint::Candidate);
            assert_eq!(recovered.len(), expected, "{}", point.as_str());
            if let Some(snapshot) = recovered.first() {
                assert_eq!(snapshot.state, ApplyOperationState::Failed);
            }
        }
        runtime.shutdown()?;
    }
    Ok(())
}
