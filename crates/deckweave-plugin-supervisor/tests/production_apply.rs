// SPDX-License-Identifier: GPL-3.0-or-later
//! Public application path through durable commit and an isolated runtime.

use deckweave_application::{
    ApiAuthScope, AppCommand, AppCommandResult, AppQuery, AppQueryResult, ApplicationGateway,
    ApplicationService, AuthorizationError, AuthorizationStore, CommandEnvelope, HostSnapshot,
    HostStatusSource, InMemoryApiAuthorizer, RequestContext,
};
use deckweave_automation::{RestrictedFixtureCompiler, VIRTUAL_FIXTURE_PACKAGE_ID};
use deckweave_domain::{
    DeploymentMode, LifecycleState,
    ids::{
        DraftId, DraftVersion, GrantId, GrantRevision, HostBootId, IdempotencyKey, PackageId,
        PrincipalId, TraceId, WorkspaceId,
    },
    security::{PermissionGrant, PermissionRequest},
};
use deckweave_host::{ApplyCoordinator, ApplyPlanningService};
use deckweave_platform_host::{NativeAtomicFileStore, NativeChildProcessPolicy};
use deckweave_plugin_supervisor::{
    host::{HostRuntimeConfig, ProductionRuntimeCoordinator},
    package::{PackageIndex, StagingLimits},
};
use deckweave_profiles::{JsonProfileStore, ProfileStoreLimits};
use deckweave_registry::CapabilityRegistry;
use deckweave_workspace::{
    ApplyCommitStatus, ApplyOperationRepository, ApplyOperationState, DraftRepository,
    PackageLockEntry, WorkspaceDraft,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const FIXTURE: &str = env!("CARGO_BIN_EXE_deckweave-virtual-plugin-fixture");

struct Status(PathBuf);

impl HostStatusSource for Status {
    fn snapshot(&self) -> HostSnapshot {
        HostSnapshot {
            lifecycle: LifecycleState::Running,
            mode: DeploymentMode::Portable,
            data_root: self.0.clone(),
            version: "test".into(),
            notice: None,
        }
    }
}

struct MutableAuthorization {
    allowed: AtomicBool,
    revision: AtomicU64,
}

impl MutableAuthorization {
    fn new() -> Self {
        Self {
            allowed: AtomicBool::new(true),
            revision: AtomicU64::new(1),
        }
    }

    fn revoke(&self) {
        self.allowed.store(false, Ordering::SeqCst);
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
}

impl AuthorizationStore for MutableAuthorization {
    fn revision(&self) -> Result<GrantRevision, AuthorizationError> {
        GrantRevision::try_new(self.revision.load(Ordering::SeqCst))
            .map_err(|error| AuthorizationError::Unavailable(error.to_string()))
    }

    fn authorize(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionGrant, AuthorizationError> {
        if !self.allowed.load(Ordering::SeqCst) {
            return Err(AuthorizationError::Denied);
        }
        Ok(PermissionGrant {
            grant_id: GrantId::try_new("test-grant")
                .map_err(|error| AuthorizationError::Unavailable(error.to_string()))?,
            revision: self.revision()?,
            request: request.clone(),
            expires_at_rfc3339: None,
            extensions: BTreeMap::new(),
        })
    }
}

fn stage_package(root: &Path) -> Result<String, Box<dyn Error>> {
    let package = root.join("packages/virtual/0.1.0/content");
    fs::create_dir_all(package.join("bin"))?;
    fs::create_dir_all(package.join("schema"))?;
    let artifact_name = format!("plugin{}", std::env::consts::EXE_SUFFIX);
    let artifact_path = package.join("bin").join(&artifact_name);
    fs::copy(FIXTURE, &artifact_path)?;
    let artifact_sha256 = digest(&fs::read(&artifact_path)?);
    let schema = br#"{"type":"object","additionalProperties":{}}"#;
    fs::write(package.join("schema/settings.json"), schema)?;
    let manifest = serde_json::json!({
        "manifestVersion": 1,
        "packageId": VIRTUAL_FIXTURE_PACKAGE_ID,
        "version": "0.1.0",
        "license": "MIT OR Apache-2.0",
        "source": {
            "repository": "https://example.invalid/deckweave/virtual",
            "revision": "production-apply-fixture"
        },
        "artifacts": [{
            "id": "current-platform",
            "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "entrypoint": format!("bin/{artifact_name}"),
            "sha256": artifact_sha256
        }],
        "runtimeGroups": [{
            "id": "control",
            "runtimeClass": "isolated_control",
            "artifactId": "current-platform"
        }],
        "contributions": [{
            "id": "virtual-device",
            "type": "org.deckweave.device_provider",
            "runtimeGroupId": "control",
            "descriptor": { "kind": "virtual" }
        }],
        "protocolRange": {
            "minimum": { "major": 1, "minor": 0 },
            "maximum": { "major": 1, "minor": 1 }
        },
        "permissions": [{
            "scope": "org.deckweave.device.virtual",
            "accessMode": "execute",
            "resourceSelector": { "deviceKind": "virtual" },
            "reason": "Publish the deterministic virtual device",
            "optional": false,
            "runtimeGroupIds": ["control"]
        }],
        "schemas": [{
            "id": "org.deckweave.fixture.settings",
            "version": { "major": 1, "minor": 0 },
            "path": "schema/settings.json",
            "sha256": digest(schema)
        }]
    });
    fs::write(
        package.join("deckweave-plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(artifact_sha256)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn draft(artifact_sha256: String) -> Result<WorkspaceDraft, Box<dyn Error>> {
    Ok(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("virtual-draft")?,
        workspace_id: WorkspaceId::try_new("virtual-workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![PackageLockEntry {
            package_id: PackageId::try_new(VIRTUAL_FIXTURE_PACKAGE_ID)?,
            version: "0.1.0".into(),
            artifact_sha256,
            source: None,
            extensions: BTreeMap::new(),
        }],
        document: serde_json::json!({"fixture": {"kind": "virtual_echo"}}),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
}

type RuntimeParts = (
    Arc<ProductionRuntimeCoordinator>,
    Arc<ApplyCoordinator>,
    Arc<Mutex<CapabilityRegistry>>,
);

fn build_runtime(
    root: &Path,
    repository: Arc<JsonProfileStore>,
    authorization: Arc<MutableAuthorization>,
    boot: &str,
) -> Result<RuntimeParts, Box<dyn Error>> {
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
        Arc::clone(&registry),
        Arc::new(NativeChildProcessPolicy),
        PrincipalId::try_new("desktop-user")?,
        HostBootId::try_new(boot)?,
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
    let apply = Arc::new(ApplyCoordinator::new(
        planning,
        Arc::clone(&repository) as Arc<dyn DraftRepository>,
        repository as Arc<dyn ApplyOperationRepository>,
        Arc::clone(&runtime) as Arc<dyn deckweave_host::RuntimeCutover>,
        16,
    )?);
    Ok((runtime, apply, registry))
}

fn gateway(
    root: &Path,
    repository: Arc<JsonProfileStore>,
    apply: Arc<ApplyCoordinator>,
) -> Result<(ApplicationGateway, RequestContext), Box<dyn Error>> {
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
    Ok((
        ApplicationGateway::new(
            Arc::new(Status(root.to_path_buf())),
            repository as Arc<dyn DraftRepository>,
            authorizer,
            apply,
        ),
        RequestContext {
            principal_id: principal,
            trace_id: TraceId::try_new("production-apply-trace")?,
        },
    ))
}

fn wait_until(mut condition: impl FnMut() -> bool) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        if Instant::now() >= deadline {
            return Err("condition did not become true before deadline".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

#[test]
fn public_apply_replays_recovers_and_revokes_without_ui() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let artifact_sha256 = stage_package(directory.path())?;
    let repository = Arc::new(JsonProfileStore::new(
        directory.path().join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    repository.create_draft(&draft(artifact_sha256)?)?;
    let authorization = Arc::new(MutableAuthorization::new());
    let (runtime, apply, registry) = build_runtime(
        directory.path(),
        Arc::clone(&repository),
        Arc::clone(&authorization),
        "boot-1",
    )?;
    let (gateway, context) = gateway(directory.path(), Arc::clone(&repository), apply)?;
    let window = gateway.open_mutation_window(&context)?;
    let plan_hash = match gateway.query(
        &context,
        AppQuery::PlanApply {
            draft_id: DraftId::try_new("virtual-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
        },
    )? {
        AppQueryResult::ApplyPlan(plan) => plan.plan_hash,
        _ => return Err("unexpected plan response".into()),
    };
    let command = CommandEnvelope {
        request_id: "apply-virtual".into(),
        idempotency_key: IdempotencyKey::try_new("apply-virtual-once")?,
        dedup_epoch: window.dedup_epoch,
        dry_run: false,
        command: AppCommand::ApplyDraft {
            draft_id: DraftId::try_new("virtual-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
            plan_hash,
        },
    };
    let operation_id = match gateway.command(&context, command.clone())? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err("unexpected apply response".into()),
    };
    let replayed_id = match gateway.command(&context, command)? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err("unexpected replay response".into()),
    };
    assert_eq!(operation_id, replayed_id);
    let operation = match gateway.query(
        &context,
        AppQuery::Operation {
            operation_id: operation_id.clone(),
        },
    )? {
        AppQueryResult::Operation(operation) => operation,
        _ => return Err("unexpected operation response".into()),
    };
    assert_eq!(operation.state, ApplyOperationState::Succeeded);
    assert_eq!(operation.commit_status, ApplyCommitStatus::Committed);
    assert_eq!(
        registry
            .lock()
            .map_err(|_| "registry poisoned")?
            .snapshot()?
            .nodes
            .len(),
        1
    );

    runtime.shutdown()?;
    assert!(
        registry
            .lock()
            .map_err(|_| "registry poisoned")?
            .snapshot()?
            .nodes
            .is_empty()
    );
    drop(gateway);
    drop(runtime);

    let (restarted, restarted_apply, restarted_registry) = build_runtime(
        directory.path(),
        Arc::clone(&repository),
        Arc::clone(&authorization),
        "boot-2",
    )?;
    let recovered = restarted_apply.recover(16)?;
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].intent.operation_id, operation_id);
    assert_eq!(
        restarted_registry
            .lock()
            .map_err(|_| "registry poisoned")?
            .snapshot()?
            .nodes
            .len(),
        1
    );

    drop(restarted_apply);
    authorization.revoke();
    wait_until(|| {
        restarted_registry
            .lock()
            .ok()
            .and_then(|registry| registry.snapshot().ok())
            .is_some_and(|snapshot| snapshot.nodes.is_empty())
    })?;
    restarted.shutdown()?;
    Ok(())
}
