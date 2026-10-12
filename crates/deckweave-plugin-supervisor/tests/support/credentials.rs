// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::{CredentialBroker, CredentialError};
use deckweave_domain::{
    ids::{CredentialLeaseId, OperationId},
    security::{CredentialLease, CredentialRequest},
};
use deckweave_host::{ApplyPlanRequest, RuntimeActivation, RuntimeCutover};

#[derive(Default)]
struct Credentials {
    resolved: AtomicBool,
    requests: Mutex<Vec<CredentialRequest>>,
}
impl CredentialBroker for Credentials {
    fn check_reference(&self, request: &CredentialRequest) -> Result<(), CredentialError> {
        self.requests
            .lock()
            .map_err(|e| CredentialError::Unavailable(e.to_string()))?
            .push(request.clone());
        if self.resolved.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(CredentialError::Unresolved)
        }
    }
    fn acquire(&self, _: &CredentialRequest) -> Result<CredentialLease, CredentialError> {
        Err(CredentialError::Unresolved)
    }
    fn revoke(&self, _: &CredentialLeaseId) -> Result<(), CredentialError> {
        Err(CredentialError::InvalidLease)
    }
}

#[test]
fn unresolved_credentials_start_no_process_and_revocation_between_prepare_and_barrier_is_checked()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let (draft, compiler, package_root) = configured_instances::configured_fixture(root, true)?;
    let repository = Arc::new(JsonProfileStore::new(
        root.join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    repository.create_draft(&draft)?;
    let packages = Arc::new(PackageIndex::scan(
        &root.join("packages"),
        &StagingLimits::default(),
    )?);
    let registry = Arc::new(Mutex::new(CapabilityRegistry::default()));
    let credentials = Arc::new(Credentials::default());
    let runtime = Arc::new(
        ProductionRuntimeCoordinator::new(
            packages,
            repository.clone(),
            Arc::new(MutableAuthorization::new()),
            registry.clone(),
            Arc::new(NativeChildProcessPolicy),
            PrincipalId::try_new("desktop-user")?,
            HostBootId::try_new("credentials")?,
            HostRuntimeConfig::default(),
        )
        .with_credential_broker(credentials.clone()),
    );
    let planning = ApplyPlanningService::new(
        repository.clone(),
        compiler,
        runtime.clone(),
        runtime.clone(),
        runtime.clone(),
        30_000,
    )?;
    let plan = planning.plan(&ApplyPlanRequest {
        draft_id: draft.draft_id,
        expected_draft_version: draft.version,
        expected_active: None,
        allow_disruption: false,
    })?;
    let operation = OperationId::try_new("credential-apply")?;
    assert!(runtime.prepare(&operation, &plan).is_err());
    assert!(!package_root.join("bin/activations.jsonl").exists());
    credentials.resolved.store(true, Ordering::SeqCst);
    let prepared = runtime.prepare(&operation, &plan)?;
    assert_eq!(
        fs::read_to_string(package_root.join("bin/activations.jsonl"))?
            .lines()
            .count(),
        2
    );
    credentials.resolved.store(false, Ordering::SeqCst);
    assert!(runtime.enter_barrier(&operation, &plan, &prepared).is_err());
    assert!(repository.active_commit()?.is_none());
    assert!(
        registry
            .lock()
            .map_err(|e| e.to_string())?
            .snapshot()?
            .nodes
            .is_empty()
    );
    runtime.abort_before_commit(&operation)?;
    runtime.shutdown()?;
    let requests = credentials.requests.lock().map_err(|e| e.to_string())?;
    for request in requests.iter() {
        assert_eq!(request.principal_id.as_str(), "desktop-user");
        assert_eq!(request.package_id.as_str(), "org.deckweave.fixture.first");
        assert_eq!(
            request
                .configured_instance_id
                .as_ref()
                .map(|id| id.as_str()),
            Some(request.credential_ref.as_str())
        );
        assert_eq!(
            request.purpose.as_str(),
            "org.deckweave.credential.plugin-use"
        );
    }
    Ok(())
}
