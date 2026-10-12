// SPDX-License-Identifier: GPL-3.0-or-later
//! Public application path through durable commit and an isolated runtime.

use deckweave_application::{
    ApiAuthScope, AppCommand, AppCommandResult, AppQuery, AppQueryResult, ApplicationGateway,
    ApplicationService, AuthorizationError, AuthorizationStore, CommandEnvelope, HostSnapshot,
    HostStatusSource, InMemoryApiAuthorizer, RequestContext,
};
use deckweave_automation::{
    CatalogEndpoint, CatalogSchema, DescriptorCatalog, EndpointAvailabilityPolicy, GraphDefinition,
    GraphDocument, GraphFormatVersion, GraphLayout, GraphLimits, GraphNode, GraphNodeKind,
    GraphPlanCompiler, GraphPort, GraphPortId, GraphPortKind, GraphPortRole, PlanCompiler,
    RestrictedFixtureCompiler, RuntimeRequirement, SchemaValueKind, VIRTUAL_FIXTURE_PACKAGE_ID,
};
use deckweave_domain::{
    DeploymentMode, LifecycleState,
    capability::NodeDescriptor,
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
    ids::{
        CapabilityId, ContributionId, DeviceIdentity, DraftId, DraftVersion, ElementId, GrantId,
        GrantRevision, GraphNodeId, HostBootId, IdempotencyKey, NodeId, PackageId, PortId,
        PrincipalId, ProviderId, RuntimeGroupId, TraceId, WorkspaceId,
    },
    security::{PermissionGrant, PermissionRequest},
};
use deckweave_host::{ApplyCoordinator, ApplyPlanningService};
use deckweave_platform_host::{NativeAtomicFileStore, NativeChildProcessPolicy};
use deckweave_plugin_supervisor::{
    host::{HostRuntimeConfig, ProductionRuntimeCoordinator, RuntimeHealthStatus},
    package::{PackageIndex, StagingLimits},
};
use deckweave_profiles::{JsonProfileStore, ProfileStoreLimits};
use deckweave_registry::CapabilityRegistry;
use deckweave_workspace::{
    ApplyCommitStatus, ApplyOperationRepository, ApplyOperationState, CommandClaimRepository,
    DraftRepository, PackageLockEntry, WorkspaceDraft,
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

#[path = "support/cleanup_receipts.rs"]
mod cleanup_receipts;
#[path = "support/configured_instances.rs"]
mod configured_instances;
#[path = "support/credentials.rs"]
mod credentials;
#[path = "support/m1_fixture.rs"]
mod m1_fixture;
#[path = "support/production_apply_crash.rs"]
mod production_apply_crash;

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
    stage_named_package(
        root,
        "virtual",
        VIRTUAL_FIXTURE_PACKAGE_ID,
        "control",
        "virtual-device",
    )
}

fn stage_named_package(
    root: &Path,
    directory: &str,
    package_id: &str,
    runtime_group: &str,
    contribution: &str,
) -> Result<String, Box<dyn Error>> {
    let package = root.join("packages").join(directory).join("0.1.0/content");
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
        "packageId": package_id,
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
            "id": runtime_group,
            "runtimeClass": "isolated_control",
            "artifactId": "current-platform"
        }],
        "contributions": [{
            "id": contribution,
            "type": "org.deckweave.device_provider",
            "runtimeGroupId": runtime_group,
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
            "runtimeGroupIds": [runtime_group]
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

fn multi_runtime_fixture(
    first_hash: String,
    second_hash: String,
) -> Result<(WorkspaceDraft, GraphPlanCompiler), Box<dyn Error>> {
    let event_schema = deckweave_domain::schema::SchemaRef {
        id: deckweave_domain::ids::NamespacedKind::try_new("org.deckweave.signal.button_edge")?,
        version: deckweave_domain::schema::SchemaVersion { major: 1, minor: 0 },
    };
    let descriptor: NodeDescriptor = serde_json::from_str(include_str!(
        "../../../protocol/fixtures/descriptors/observe-keyboard.node.json"
    ))?;
    let port = descriptor.devices[0].elements[0].capabilities[0].ports[0].clone();
    let entries = [
        (
            "org.deckweave.fixture.first",
            "first-group",
            "first-device",
            first_hash.clone(),
        ),
        (
            "org.deckweave.fixture.second",
            "second-group",
            "second-device",
            second_hash.clone(),
        ),
    ];
    let mut catalog_endpoints = Vec::new();
    let mut nodes = Vec::new();
    let mut package_lock = Vec::new();
    for (index, (package, group, contribution, hash)) in entries.into_iter().enumerate() {
        let endpoint = observed_key_endpoint(package)?;
        let requirement = RuntimeRequirement {
            configuration: None,
            package_id: endpoint.package_id.clone(),
            package_version: "0.1.0".into(),
            artifact_sha256: hash.clone(),
            runtime_group_id: RuntimeGroupId::try_new(group)?,
            contribution_id: ContributionId::try_new(contribution)?,
        };
        catalog_endpoints.push(CatalogEndpoint {
            endpoint: endpoint.clone(),
            runtime_requirement: requirement,
            port: port.clone(),
            operation: None,
        });
        nodes.push(GraphNode {
            id: GraphNodeId::try_new(format!("source-{index}"))?,
            kind: GraphNodeKind::Source {
                endpoint,
                availability: EndpointAvailabilityPolicy::Deferred,
            },
            inputs: vec![],
            outputs: vec![GraphPort {
                id: GraphPortId::try_new("edge")?,
                kind: GraphPortKind::Flow,
                role: GraphPortRole::Trigger,
                schema: event_schema.clone(),
                unit: None,
                range: None,
            }],
            extensions: BTreeMap::new(),
        });
        package_lock.push(PackageLockEntry {
            package_id: PackageId::try_new(package)?,
            version: "0.1.0".into(),
            artifact_sha256: hash,
            source: None,
            extensions: BTreeMap::new(),
        });
    }
    let graph = GraphDocument {
        plugin_instances: Default::default(),
        schema_version: GraphFormatVersion { major: 1, minor: 0 },
        graph: GraphDefinition {
            node_instances: Default::default(),
            action_arguments: Default::default(),
            nodes,
            edges: vec![],
            extensions: BTreeMap::new(),
        },
        layout: GraphLayout::default(),
        extensions: BTreeMap::new(),
    };
    let compiler = GraphPlanCompiler::new(
        DescriptorCatalog::try_new(
            vec![CatalogSchema {
                schema: event_schema,
                value_kind: SchemaValueKind::Json,
                unit: None,
                range: None,
            }],
            catalog_endpoints,
        )?,
        GraphLimits::default(),
    )?;
    Ok((
        WorkspaceDraft {
            schema_version: 1,
            draft_id: DraftId::try_new("multi-runtime-draft")?,
            workspace_id: WorkspaceId::try_new("multi-runtime-workspace")?,
            base_revision_id: None,
            version: DraftVersion::try_new(1)?,
            package_lock,
            document: serde_json::to_value(graph)?,
            assets: vec![],
            private_section: None,
            extensions: BTreeMap::new(),
            unknown_fields: BTreeMap::new(),
        },
        compiler,
    ))
}

fn observed_key_endpoint(package: &str) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new(package)?,
        node_id: NodeId::try_new("local-node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("fixture-hid-provider")?,
            device_identity: DeviceIdentity::try_new("keyboard-observe-fixture")?,
        },
        port: PortPath {
            element_id: ElementId::try_new("key-a")?,
            capability_id: CapabilityId::try_new("key-a-button")?,
            port_id: PortId::try_new("key-a.edge")?,
        },
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
    build_runtime_with_compiler(
        root,
        repository,
        authorization,
        boot,
        Arc::new(RestrictedFixtureCompiler),
    )
}

fn build_runtime_with_compiler(
    root: &Path,
    repository: Arc<JsonProfileStore>,
    authorization: Arc<MutableAuthorization>,
    boot: &str,
    compiler: Arc<dyn PlanCompiler>,
) -> Result<RuntimeParts, Box<dyn Error>> {
    let packages = Arc::new(PackageIndex::scan(
        &root.join("packages"),
        &StagingLimits::default(),
    )?);
    let registry = Arc::new(Mutex::new(CapabilityRegistry::default()));
    let runtime = Arc::new(ProductionRuntimeCoordinator::new(
        packages,
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
        compiler,
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
    runtime: Arc<ProductionRuntimeCoordinator>,
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
            Arc::clone(&repository) as Arc<dyn DraftRepository>,
            repository as Arc<dyn CommandClaimRepository>,
            authorizer,
            apply,
        )
        .with_runtime_diagnostics(runtime),
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
fn graph_apply_binds_and_activates_two_runtime_groups() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let first_hash = stage_named_package(
        directory.path(),
        "first",
        "org.deckweave.fixture.first",
        "first-group",
        "first-device",
    )?;
    let second_hash = stage_named_package(
        directory.path(),
        "second",
        "org.deckweave.fixture.second",
        "second-group",
        "second-device",
    )?;
    let (draft, compiler) = multi_runtime_fixture(first_hash, second_hash)?;
    let repository = Arc::new(JsonProfileStore::new(
        directory.path().join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    repository.create_draft(&draft)?;
    let authorization = Arc::new(MutableAuthorization::new());
    let (runtime, apply, registry) = build_runtime_with_compiler(
        directory.path(),
        Arc::clone(&repository),
        authorization,
        "multi-boot",
        Arc::new(compiler),
    )?;
    let (gateway, context) = gateway(
        directory.path(),
        Arc::clone(&repository),
        apply,
        Arc::clone(&runtime),
    )?;
    let window = gateway.open_mutation_window(&context)?;
    let plan_hash = match gateway.query(
        &context,
        AppQuery::PlanApply {
            draft_id: DraftId::try_new("multi-runtime-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
            allow_disruption: false,
        },
    )? {
        AppQueryResult::ApplyPlan(plan) => plan.plan_hash,
        _ => return Err("unexpected plan response".into()),
    };
    let operation_id = match gateway.command(
        &context,
        CommandEnvelope {
            request_id: "apply-multi-runtime".into(),
            idempotency_key: IdempotencyKey::try_new("apply-multi-runtime")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("multi-runtime-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: false,
                plan_hash,
            },
        },
    )? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err("unexpected apply response".into()),
    };
    let operation = match gateway.query(&context, AppQuery::Operation { operation_id })? {
        AppQueryResult::Operation(operation) => operation,
        _ => return Err("unexpected operation response".into()),
    };
    assert_eq!(operation.state, ApplyOperationState::Succeeded);
    let snapshot = registry
        .lock()
        .map_err(|_| "registry poisoned")?
        .snapshot()?;
    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(
        runtime
            .runtime_health()
            .runtimes
            .iter()
            .filter(|entry| entry.status == RuntimeHealthStatus::Active)
            .count(),
        2
    );
    let packages: std::collections::BTreeSet<_> = snapshot
        .nodes
        .into_iter()
        .map(|node| node.session.package_id.to_string())
        .collect();
    assert_eq!(
        packages,
        [
            "org.deckweave.fixture.first".to_owned(),
            "org.deckweave.fixture.second".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn m1_runs_isolated_source_action_and_feedback_end_to_end() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (draft, compiler, feedback_marker) = m1_fixture::stage_and_build(directory.path())?;
    let repository = Arc::new(JsonProfileStore::new(
        directory.path().join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    repository.create_draft(&draft)?;
    let authorization = Arc::new(MutableAuthorization::new());
    let (runtime, apply, registry) = build_runtime_with_compiler(
        directory.path(),
        Arc::clone(&repository),
        authorization,
        "m1-boot",
        Arc::new(compiler),
    )?;
    let (gateway, context) = gateway(
        directory.path(),
        Arc::clone(&repository),
        apply,
        Arc::clone(&runtime),
    )?;
    let window = gateway.open_mutation_window(&context)?;
    let plan_hash = match gateway.query(
        &context,
        AppQuery::PlanApply {
            draft_id: DraftId::try_new("m1-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
            allow_disruption: false,
        },
    )? {
        AppQueryResult::ApplyPlan(plan) => plan.plan_hash,
        _ => return Err("unexpected M1 plan response".into()),
    };
    let operation_id = match gateway.command(
        &context,
        CommandEnvelope {
            request_id: "apply-m1".into(),
            idempotency_key: IdempotencyKey::try_new("apply-m1-once")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: DraftId::try_new("m1-draft")?,
                expected_draft_version: DraftVersion::try_new(1)?,
                expected_active: None,
                allow_disruption: false,
                plan_hash,
            },
        },
    )? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err("unexpected M1 apply response".into()),
    };
    wait_until(|| {
        runtime
            .live_execution()
            .snapshot()
            .ok()
            .flatten()
            .is_some_and(|state| state.pending.is_empty() && state.completed.len() == 2)
            && fs::read_to_string(&feedback_marker)
                .ok()
                .is_some_and(|value| value.lines().count() == 2)
    })?;
    let operation = match gateway.query(&context, AppQuery::Operation { operation_id })? {
        AppQueryResult::Operation(operation) => operation,
        _ => return Err("unexpected M1 operation response".into()),
    };
    assert_eq!(operation.state, ApplyOperationState::Succeeded);
    let trace = match gateway.query(&context, AppQuery::RuntimeTrace)? {
        AppQueryResult::RuntimeTrace(trace) => trace,
        _ => return Err("unexpected M1 trace response".into()),
    };
    assert_eq!(trace.format_version, 1);
    assert!(trace.plan_hash.is_some());
    assert!(!trace.schema_hashes.is_empty());
    assert_eq!(
        trace
            .entries
            .iter()
            .filter(|entry| entry.kind == deckweave_application::RuntimeTraceEventKind::IntentInvoke)
            .count(),
        2
    );
    assert_eq!(
        trace
            .entries
            .iter()
            .filter(
                |entry| entry.kind == deckweave_application::RuntimeTraceEventKind::IntentFeedback
            )
            .count(),
        2
    );
    let scheduler = runtime
        .live_execution()
        .snapshot()?
        .ok_or("M1 scheduler is not active")?;
    assert_eq!(scheduler.pending.len(), 0);
    assert_eq!(scheduler.completed.len(), 2);
    assert!(
        scheduler
            .values
            .get(&GraphNodeId::try_new("result-state")?)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("completed:run-"))
    );
    let registry = registry
        .lock()
        .map_err(|_| "registry poisoned")?
        .snapshot()?;
    assert_eq!(registry.nodes.len(), 3);
    assert!(
        registry
            .nodes
            .iter()
            .any(|node| node.descriptor.devices.len() == 2)
    );
    assert_eq!(
        runtime
            .runtime_health()
            .runtimes
            .iter()
            .filter(|entry| entry.status == RuntimeHealthStatus::Active)
            .count(),
        3
    );
    runtime.shutdown()?;
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
    let (gateway, context) = gateway(
        directory.path(),
        Arc::clone(&repository),
        apply,
        Arc::clone(&runtime),
    )?;
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
    let command = CommandEnvelope {
        request_id: "apply-virtual".into(),
        idempotency_key: IdempotencyKey::try_new("apply-virtual-once")?,
        dedup_epoch: window.dedup_epoch,
        dry_run: false,
        command: AppCommand::ApplyDraft {
            draft_id: DraftId::try_new("virtual-draft")?,
            expected_draft_version: DraftVersion::try_new(1)?,
            expected_active: None,
            allow_disruption: false,
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
    wait_until(|| {
        restarted
            .runtime_health()
            .runtimes
            .iter()
            .all(|entry| entry.status == RuntimeHealthStatus::Unavailable)
    })?;
    let historical = repository.operation(&operation_id)?;
    assert_eq!(historical.state, ApplyOperationState::Succeeded);
    assert_eq!(historical.commit_status, ApplyCommitStatus::Committed);
    restarted.shutdown()?;
    Ok(())
}
