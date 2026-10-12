// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_automation::{
    ExecutionPlanKind, RestrictedFixtureCompiler, VIRTUAL_FIXTURE_PACKAGE_ID,
};
use deckweave_domain::ids::{
    DedupEpoch, DraftId, DraftVersion, GrantRevision, HostBootId, OperationId, PackageId,
    PrincipalId, RegistryRevision, WorkspaceId,
};
use deckweave_host::{
    ActiveSelection, ApplyPlan, ApplyPlanRequest, ApplyPlanningError, ApplyPlanningService,
    PlanPrecondition, PlanningClock, PlanningEvidence, PlanningEvidenceError,
    PlanningEvidenceSource, PreparedActivation, RuntimeActivation, RuntimeActivationError,
};
use deckweave_workspace::{
    DraftCommand, DraftRepository, DraftRepositoryError, DraftView, MutationKey, MutationWindow,
    PackageLockEntry, StoredDraftMutation, WorkspaceCandidate, WorkspaceDraft,
};
use std::{
    collections::BTreeMap,
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

struct CandidateRepository {
    candidate: Mutex<WorkspaceCandidate>,
}

impl CandidateRepository {
    fn replace(&self, candidate: WorkspaceCandidate) {
        *self
            .candidate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = candidate;
    }
}

impl DraftRepository for CandidateRepository {
    fn open_mutation_window(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<MutationWindow, DraftRepositoryError> {
        Ok(MutationWindow {
            principal_id: principal_id.clone(),
            dedup_epoch: DedupEpoch::try_new("unused")
                .map_err(|error| DraftRepositoryError::Unavailable(error.to_string()))?,
            issued_at_unix_seconds: 1,
            expires_at_unix_seconds: 2,
        })
    }

    fn get_draft(&self, _draft_id: &DraftId) -> Result<DraftView, DraftRepositoryError> {
        Err(DraftRepositoryError::Unavailable(
            "unused in planning".into(),
        ))
    }

    fn get_candidate(
        &self,
        draft_id: &DraftId,
    ) -> Result<WorkspaceCandidate, DraftRepositoryError> {
        let candidate = self
            .candidate
            .lock()
            .map_err(|_| DraftRepositoryError::Unavailable("candidate lock poisoned".into()))?
            .clone();
        if candidate.draft_id() != draft_id {
            return Err(DraftRepositoryError::NotFound);
        }
        Ok(candidate)
    }

    fn preview_draft_mutation(
        &self,
        _draft_id: &DraftId,
        _expected_version: DraftVersion,
        _command: &DraftCommand,
    ) -> Result<DraftView, DraftRepositoryError> {
        Err(DraftRepositoryError::Unavailable(
            "unused in planning".into(),
        ))
    }

    fn mutate_draft(
        &self,
        _draft_id: &DraftId,
        _key: MutationKey,
        _expected_version: DraftVersion,
        _command: &DraftCommand,
    ) -> Result<StoredDraftMutation, DraftRepositoryError> {
        Err(DraftRepositoryError::Unavailable(
            "unused in planning".into(),
        ))
    }
}

struct EvidenceSource {
    evidence: Mutex<PlanningEvidence>,
}

impl EvidenceSource {
    fn update(&self, update: impl FnOnce(&mut PlanningEvidence)) {
        update(
            &mut self
                .evidence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
}

impl PlanningEvidenceSource for EvidenceSource {
    fn snapshot(&self) -> Result<PlanningEvidence, PlanningEvidenceError> {
        self.evidence
            .lock()
            .map_err(|_| PlanningEvidenceError::Unavailable("evidence lock poisoned".into()))
            .map(|evidence| evidence.clone())
    }
}

struct TestClock(AtomicU64);

impl PlanningClock for TestClock {
    fn monotonic_millis(&self) -> Result<u64, ApplyPlanningError> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

struct TestActivation {
    calls: AtomicUsize,
}

impl RuntimeActivation for TestActivation {
    fn prepare(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
    ) -> Result<PreparedActivation, RuntimeActivationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(PreparedActivation::new(
            operation_id.clone(),
            plan.token().plan_hash().into(),
        ))
    }
}

type Fixture = (
    ApplyPlanningService,
    Arc<CandidateRepository>,
    Arc<EvidenceSource>,
    Arc<TestClock>,
    Arc<TestActivation>,
    ApplyPlanRequest,
);

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let candidate = candidate(serde_json::json!({}), 1)?;
    let draft_id = candidate.draft_id().clone();
    let repository = Arc::new(CandidateRepository {
        candidate: Mutex::new(candidate),
    });
    let evidence = Arc::new(EvidenceSource {
        evidence: Mutex::new(PlanningEvidence {
            host_boot_id: HostBootId::try_new("boot-1")?,
            registry_revision: RegistryRevision::try_new(1)?,
            grant_revision: GrantRevision::try_new(1)?,
            active: None,
        }),
    });
    let clock = Arc::new(TestClock(AtomicU64::new(100)));
    let activation = Arc::new(TestActivation {
        calls: AtomicUsize::new(0),
    });
    let service = ApplyPlanningService::new(
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        Arc::new(RestrictedFixtureCompiler),
        Arc::clone(&evidence) as Arc<dyn PlanningEvidenceSource>,
        Arc::clone(&activation) as Arc<dyn RuntimeActivation>,
        Arc::clone(&clock) as Arc<dyn PlanningClock>,
        50,
    )?;
    let request = ApplyPlanRequest {
        draft_id,
        expected_draft_version: DraftVersion::try_new(1)?,
        expected_active: None,
    };
    Ok((service, repository, evidence, clock, activation, request))
}

fn candidate(
    document: serde_json::Value,
    version: u64,
) -> Result<WorkspaceCandidate, Box<dyn Error>> {
    WorkspaceCandidate::from_draft(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("planning-draft")?,
        workspace_id: WorkspaceId::try_new("planning-workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(version)?,
        package_lock: vec![],
        document,
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
    .map_err(Into::into)
}

fn virtual_candidate(artifact: char) -> Result<WorkspaceCandidate, Box<dyn Error>> {
    let mut draft = WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("planning-draft")?,
        workspace_id: WorkspaceId::try_new("planning-workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document: serde_json::json!({"fixture": {"kind": "virtual_echo"}}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    };
    draft.package_lock.push(PackageLockEntry {
        package_id: PackageId::try_new(VIRTUAL_FIXTURE_PACKAGE_ID)?,
        version: "0.1.0".into(),
        artifact_sha256: artifact.to_string().repeat(64),
        source: None,
        extensions: BTreeMap::new(),
    });
    WorkspaceCandidate::from_draft(draft).map_err(Into::into)
}

fn assert_precondition(result: Result<(), ApplyPlanningError>, expected: PlanPrecondition) {
    assert!(matches!(
        result,
        Err(ApplyPlanningError::PreconditionChanged(actual)) if actual == expected
    ));
}

#[test]
fn prepare_uses_a_fenced_plan_and_barrier_rechecks_grants() -> Result<(), Box<dyn Error>> {
    let (service, _, evidence, _, activation, request) = fixture()?;
    let plan = service.plan(&request)?;
    assert_eq!(plan.compiled().kind(), ExecutionPlanKind::EmptyFixture);
    assert_eq!(plan.token().plan_hash().len(), 64);
    let operation_id = OperationId::try_new("apply-1")?;
    let prepared = service.prepare(&operation_id, &plan)?;
    assert_eq!(prepared.operation_id(), &operation_id);
    assert_eq!(activation.calls.load(Ordering::SeqCst), 1);

    let next_grant_revision = GrantRevision::try_new(2)?;
    evidence.update(move |evidence| {
        evidence.grant_revision = next_grant_revision;
    });
    assert_precondition(
        service.revalidate_for_barrier(&plan, &prepared),
        PlanPrecondition::GrantRevision,
    );
    Ok(())
}

#[test]
fn changed_topology_or_candidate_blocks_prepare_before_activation() -> Result<(), Box<dyn Error>> {
    let (service, _, evidence, _, activation, request) = fixture()?;
    let plan = service.plan(&request)?;
    let next_registry_revision = RegistryRevision::try_new(2)?;
    evidence.update(move |evidence| {
        evidence.registry_revision = next_registry_revision;
    });
    let error = service
        .prepare(&OperationId::try_new("apply-topology")?, &plan)
        .err()
        .ok_or_else(|| std::io::Error::other("changed topology unexpectedly prepared"))?;
    assert!(matches!(
        error,
        ApplyPlanningError::PreconditionChanged(PlanPrecondition::RegistryRevision)
    ));
    assert_eq!(activation.calls.load(Ordering::SeqCst), 0);

    let (service, repository_again, _, _, activation_again, request) = fixture()?;
    let plan = service.plan(&request)?;
    repository_again.replace(candidate(serde_json::json!({"changed": true}), 1)?);
    let error = service
        .prepare(&OperationId::try_new("apply-candidate")?, &plan)
        .err()
        .ok_or_else(|| std::io::Error::other("changed candidate unexpectedly prepared"))?;
    assert!(matches!(
        error,
        ApplyPlanningError::PreconditionChanged(PlanPrecondition::CandidateContent)
    ));
    assert_eq!(activation_again.calls.load(Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn expired_plan_and_changed_active_selection_are_rejected() -> Result<(), Box<dyn Error>> {
    let (service, _, _, clock, _, request) = fixture()?;
    let plan = service.plan(&request)?;
    clock.0.store(150, Ordering::SeqCst);
    let error = service
        .prepare(&OperationId::try_new("apply-expired")?, &plan)
        .err()
        .ok_or_else(|| std::io::Error::other("expired plan unexpectedly prepared"))?;
    assert!(matches!(
        error,
        ApplyPlanningError::PreconditionChanged(PlanPrecondition::Expired)
    ));

    let (service, _, evidence, _, _, request) = fixture()?;
    let changed_active = ActiveSelection {
        workspace_id: WorkspaceId::try_new("other-workspace")?,
        revision_id: deckweave_domain::ids::WorkspaceRevisionId::try_new("revision-2")?,
    };
    evidence.update(move |evidence| {
        evidence.active = Some(changed_active);
    });
    let error = service
        .plan(&request)
        .err()
        .ok_or_else(|| std::io::Error::other("changed active selection unexpectedly planned"))?;
    assert!(matches!(
        error,
        ApplyPlanningError::PreconditionChanged(PlanPrecondition::ActiveSelection)
    ));
    Ok(())
}

#[test]
fn package_lock_change_invalidates_a_virtual_fixture_plan() -> Result<(), Box<dyn Error>> {
    let (service, repository, _, _, activation, request) = fixture()?;
    repository.replace(virtual_candidate('a')?);
    let plan = service.plan(&request)?;
    assert_eq!(
        plan.compiled().kind(),
        ExecutionPlanKind::VirtualEchoFixture
    );

    repository.replace(virtual_candidate('b')?);
    let error = service
        .prepare(&OperationId::try_new("apply-package-change")?, &plan)
        .err()
        .ok_or_else(|| std::io::Error::other("changed package lock unexpectedly prepared"))?;
    assert!(matches!(
        error,
        ApplyPlanningError::PreconditionChanged(PlanPrecondition::PackageLock)
    ));
    assert_eq!(activation.calls.load(Ordering::SeqCst), 0);
    Ok(())
}
