// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{AuthorizationError, AuthorizationStore};
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        DescriptorRevision, GrantId, GrantRevision, PluginSessionId, PrincipalId, RuntimeGroupId,
    },
    security::{PermissionGrant, PermissionRequest},
};
use deckweave_platform_host::NativeChildProcessPolicy;
use deckweave_platform_windows::PlatformFailure;
use deckweave_plugin_supervisor::{
    package::{IndexedPackage, PackageStager, StagingLimits},
    process::{ProcessConfig, ProcessRuntime},
    supervisor::{
        RestartPolicy, RuntimeController, RuntimeLeaseReason, RuntimeStatus, SupervisorAction,
    },
};
use deckweave_protocol::{
    FEATURE_ACTIVATION_COMMIT, FEATURE_INVOCATION_ENDPOINT_PATH, HostProtocolSupport,
    negotiate_hello,
    v1::{
        ActivationCommitted, ActivationRequest, DeactivationRequest, EndpointPath, ErrorCategory,
        InvocationRequest, ProtocolEnvelope, ProtocolLimits, ProtocolVersion, ProtocolVersionRange,
        protocol_envelope,
    },
};
use deckweave_registry::CapabilityRegistry;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

const CONTROL: &str = env!("CARGO_BIN_EXE_deckweave-platform-windows-control");
const INPUT: &str = env!("CARGO_BIN_EXE_deckweave-platform-windows-input-observe");
const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn stage(root: &Path) -> Result<IndexedPackage, Box<dyn Error>> {
    fs::create_dir_all(root.join("bin"))?;
    fs::create_dir_all(root.join("schemas"))?;
    let suffix = std::env::consts::EXE_SUFFIX;
    let artifacts = [
        (
            "windows-x86_64-control",
            CONTROL,
            format!("bin/deckweave-platform-windows-control{suffix}"),
        ),
        (
            "windows-x86_64-input-observe",
            INPUT,
            format!("bin/deckweave-platform-windows-input-observe{suffix}"),
        ),
    ];
    let mut manifest: Value = serde_json::from_slice(&fs::read(
        Path::new(ROOT).join("package/deckweave-plugin.template.json"),
    )?)?;
    for (index, (id, source, relative)) in artifacts.iter().enumerate() {
        let bytes = fs::read(source)?;
        let destination = root.join(relative);
        fs::copy(source, &destination)?;
        manifest["artifacts"][index]["id"] = Value::String((*id).into());
        manifest["artifacts"][index]["os"] = Value::String(std::env::consts::OS.into());
        manifest["artifacts"][index]["architecture"] = Value::String(std::env::consts::ARCH.into());
        manifest["artifacts"][index]["entrypoint"] = Value::String(relative.clone());
        manifest["artifacts"][index]["sha256"] = Value::String(digest(&bytes));
    }
    let schemas = manifest["schemas"]
        .as_array_mut()
        .ok_or("manifest schemas are not an array")?;
    for schema in schemas {
        let relative = schema["path"].as_str().ok_or("schema path")?;
        let bytes = fs::read(Path::new(ROOT).join(relative))?;
        fs::write(root.join(relative), &bytes)?;
        schema["sha256"] = Value::String(digest(&bytes));
    }
    fs::write(
        root.join("deckweave-plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(PackageStager::inspect(root, &StagingLimits::default())?.package)
}

fn support() -> HostProtocolSupport {
    HostProtocolSupport {
        protocol: ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        },
        features: BTreeMap::from([
            (FEATURE_ACTIVATION_COMMIT.into(), 1),
            (FEATURE_INVOCATION_ENDPOINT_PATH.into(), 1),
        ]),
        schemas: BTreeMap::new(),
        limits: ProtocolLimits {
            maximum_message_bytes: 64 * 1024,
            maximum_chunk_bytes: 64 * 1024,
            maximum_concurrent_jobs: 8,
            maximum_artifact_bytes: 1024 * 1024,
            maximum_events_per_second: 512,
        },
    }
}

fn envelope(message_id: u64, body: protocol_envelope::Body) -> ProtocolEnvelope {
    ProtocolEnvelope {
        wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
        session_generation: 1,
        message_id,
        trace: None,
        body: Some(body),
    }
}

struct ScopedAuthorization {
    deny_input: AtomicBool,
    revision: AtomicU64,
}

impl ScopedAuthorization {
    fn new() -> Self {
        Self {
            deny_input: AtomicBool::new(false),
            revision: AtomicU64::new(1),
        }
    }

    fn revoke_input(&self) {
        self.deny_input.store(true, Ordering::SeqCst);
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
}

impl AuthorizationStore for ScopedAuthorization {
    fn revision(&self) -> Result<GrantRevision, AuthorizationError> {
        GrantRevision::try_new(self.revision.load(Ordering::SeqCst))
            .map_err(|error| AuthorizationError::Unavailable(error.to_string()))
    }

    fn authorize(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionGrant, AuthorizationError> {
        if self.deny_input.load(Ordering::SeqCst) && request.scope.as_str() == "input.observe" {
            return Err(AuthorizationError::Denied);
        }
        Ok(PermissionGrant {
            grant_id: GrantId::try_new("windows-test-grant")
                .map_err(|error| AuthorizationError::Unavailable(error.to_string()))?,
            revision: self.revision()?,
            request: request.clone(),
            expires_at_rfc3339: None,
            extensions: BTreeMap::new(),
        })
    }
}

fn runtime_hello(group: &str, token: &[u8]) -> deckweave_protocol::v1::Hello {
    let mut features = vec![deckweave_protocol::v1::FeatureRequirement {
        id: FEATURE_ACTIVATION_COMMIT.into(),
        minimum_version: 1,
        maximum_version: 1,
        required: true,
    }];
    if group == "control" {
        features.push(deckweave_protocol::v1::FeatureRequirement {
            id: FEATURE_INVOCATION_ENDPOINT_PATH.into(),
            minimum_version: 1,
            maximum_version: 1,
            required: true,
        });
    }
    deckweave_protocol::v1::Hello {
        protocol: Some(ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        }),
        package_id: "org.deckweave.platform.windows".into(),
        runtime_group: group.into(),
        one_time_token: token.to_vec(),
        features,
        schemas: vec![],
        limits: Some(support().limits),
    }
}

fn activate_controller(
    controller: &mut RuntimeController,
    group: &str,
    nodes: Vec<NodeDescriptor>,
    authorization: &ScopedAuthorization,
    registry: &mut CapabilityRegistry,
) -> Result<(), Box<dyn Error>> {
    controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    assert_eq!(
        controller.check_permissions(&PrincipalId::try_new("local-user")?, authorization)?,
        SupervisorAction::StartProcess
    );
    let token = vec![9; 32];
    controller.process_started(
        PluginSessionId::try_new(format!("session-{group}"))?,
        token.clone(),
        1,
    )?;
    controller.accept_hello(&runtime_hello(group, &token), &support())?;
    controller.activate_descriptors(registry, DescriptorRevision::try_new(1)?, nodes, 2)?;
    Ok(())
}

fn exercise(
    package: &IndexedPackage,
    group: &str,
    contribution: &str,
    invoke: bool,
) -> Result<Vec<NodeDescriptor>, Box<dyn Error>> {
    let group_id = RuntimeGroupId::try_new(group)?;
    let mut process = ProcessRuntime::spawn_indexed(
        package,
        &group_id,
        &[7; 32],
        1,
        ProcessConfig::default(),
        &BTreeMap::new(),
        &NativeChildProcessPolicy,
    )?;
    let hello = process.receive(Duration::from_secs(2))?;
    let Some(protocol_envelope::Body::Hello(hello)) = hello.body else {
        return Err("expected hello".into());
    };
    assert_eq!(hello.package_id, "org.deckweave.platform.windows");
    assert_eq!(hello.runtime_group, group);
    assert_eq!(hello.one_time_token, [7; 32]);
    let ack = negotiate_hello(&hello, &support(), format!("session-{group}"))?;
    process.send(&envelope(1, protocol_envelope::Body::HelloAck(ack)))?;
    process.send(&envelope(
        2,
        protocol_envelope::Body::Activate(ActivationRequest {
            runtime_lease_id: format!("lease-{group}"),
            contribution_id: contribution.into(),
            settings: None,
            locale: None,
            configured_instance_id: None,
            settings_revision: 0,
            credential_refs: Default::default(),
        }),
    ))?;
    let activated = process.receive(Duration::from_secs(2))?;
    let Some(protocol_envelope::Body::Activated(activated)) = activated.body else {
        return Err("expected activation reply".into());
    };
    let nodes: Vec<NodeDescriptor> =
        serde_json::from_slice(&activated.descriptor.ok_or("missing descriptor")?.value)?;
    for node in &nodes {
        node.validate()?;
    }
    let heartbeat = process.receive(Duration::from_secs(2))?;
    assert!(matches!(
        heartbeat.body,
        Some(protocol_envelope::Body::Heartbeat(_))
    ));
    process.send(&envelope(
        3,
        protocol_envelope::Body::ActivationCommitted(ActivationCommitted {
            activation_epoch: 1,
            host_ticks: 1,
        }),
    ))?;
    if invoke {
        process.send(&envelope(
            4,
            protocol_envelope::Body::Invoke(InvocationRequest {
                invocation_id: "controlled-1".into(),
                run_id: "run-1".into(),
                operation_id: "set-volume".into(),
                activation_epoch: 1,
                descriptor_revision: 1,
                deadline_host_ticks: 100,
                idempotency_key: None,
                input: None,
                target: Some(EndpointPath {
                    node_id: "windows-control-node".into(),
                    provider_id: "org.deckweave.platform.windows.control".into(),
                    target: Some(deckweave_protocol::v1::endpoint_path::Target::ServiceId(
                        "windows-platform".into(),
                    )),
                    element_id: "audio".into(),
                    capability_id: "endpoint-control".into(),
                    port_id: "set-volume".into(),
                }),
                remaining_budget_ms: Some(1000),
            }),
        ))?;
        let reply = process.receive(Duration::from_secs(2))?;
        let Some(protocol_envelope::Body::InvocationReply(reply)) = reply.body else {
            return Err("expected invocation rejection".into());
        };
        let Some(deckweave_protocol::v1::invocation_reply::Reply::Rejected(error)) = reply.reply
        else {
            return Err("controlled adapter must reject invocation".into());
        };
        assert_eq!(
            error.code,
            PlatformFailure::controlled_adapter().structured().code
        );
    }
    process.send(&envelope(
        5,
        protocol_envelope::Body::Deactivate(DeactivationRequest {
            runtime_lease_id: format!("lease-{group}"),
            reason: "test".into(),
        }),
    ))?;
    let status = process
        .wait_timeout(Duration::from_secs(2))?
        .ok_or("runtime did not stop")?;
    assert!(status.success());
    Ok(nodes)
}

#[test]
fn package_indexes_two_permission_scoped_runtime_groups() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let package = stage(temp.path())?;
    assert_eq!(package.runtime_groups.len(), 2);
    assert_eq!(package.contributions.len(), 2);
    let by_group: BTreeMap<_, BTreeSet<_>> = package
        .requested_permissions
        .iter()
        .flat_map(|permission| {
            permission
                .runtime_group_ids
                .iter()
                .map(|group| (group.to_string(), permission.scope.to_string()))
        })
        .fold(BTreeMap::new(), |mut groups, (group, scope)| {
            groups
                .entry(group)
                .or_insert_with(BTreeSet::new)
                .insert(scope);
            groups
        });
    assert_eq!(
        by_group["input-observe"],
        BTreeSet::from(["input.observe".into()])
    );
    assert!(!by_group["control"].contains("input.observe"));
    Ok(())
}

#[test]
fn revoking_input_observe_does_not_stop_the_control_group() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let package = Arc::new(stage(temp.path())?);
    let authorization = ScopedAuthorization::new();
    let mut registry = CapabilityRegistry::default();
    let mut control = RuntimeController::new(
        Arc::clone(&package),
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut input = RuntimeController::new(
        package,
        RuntimeGroupId::try_new("input-observe")?,
        RestartPolicy::default(),
    )?;
    activate_controller(
        &mut control,
        "control",
        serde_json::from_str(include_str!("../assets/control-descriptors.json"))?,
        &authorization,
        &mut registry,
    )?;
    activate_controller(
        &mut input,
        "input-observe",
        serde_json::from_str(include_str!("../assets/input-observe-descriptors.json"))?,
        &authorization,
        &mut registry,
    )?;
    authorization.revoke_input();
    assert_eq!(
        input.revalidate_permissions(
            &PrincipalId::try_new("local-user")?,
            &authorization,
            &mut registry,
            10,
        )?,
        SupervisorAction::RequestGracefulStop
    );
    assert_eq!(
        control.revalidate_permissions(
            &PrincipalId::try_new("local-user")?,
            &authorization,
            &mut registry,
            10,
        )?,
        SupervisorAction::None
    );
    assert_eq!(input.status(), RuntimeStatus::Stopping);
    assert_eq!(control.status(), RuntimeStatus::Active);
    let snapshot = registry.snapshot()?;
    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(
        snapshot.nodes[0].descriptor.node_id.as_str(),
        "windows-control-node"
    );
    Ok(())
}

#[test]
fn both_group_shells_run_through_the_production_process_boundary() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let package = stage(temp.path())?;
    let control = exercise(&package, "control", "windows-control", true)?;
    let input = exercise(&package, "input-observe", "keyboard-observe", false)?;
    assert_eq!(control[0].node_id.as_str(), "windows-control-node");
    assert_eq!(input[0].node_id.as_str(), "windows-input-node");
    Ok(())
}

#[test]
fn descriptor_paths_scope_repeated_local_port_ids() -> Result<(), Box<dyn Error>> {
    let nodes: Vec<NodeDescriptor> =
        serde_json::from_str(include_str!("../assets/control-descriptors.json"))?;
    let mut paths = BTreeSet::new();
    let mut repeated = 0;
    for service in &nodes[0].services {
        for element in &service.elements {
            for capability in &element.capabilities {
                for port in &capability.ports {
                    if port.port_id.as_str() == "invoke" {
                        repeated += 1;
                    }
                    assert!(paths.insert((
                        service.service_id.to_string(),
                        element.element_id.to_string(),
                        capability.capability_id.to_string(),
                        port.port_id.to_string(),
                    )));
                }
            }
        }
    }
    assert_eq!(repeated, 3);
    Ok(())
}

#[test]
fn platform_failures_have_stable_wire_categories_and_codes() {
    let cases = [
        (
            PlatformFailure::UnsupportedOperatingSystem,
            ErrorCategory::Unsupported,
            "unsupported_operating_system",
        ),
        (
            PlatformFailure::UnsupportedBuild,
            ErrorCategory::Unsupported,
            "unsupported_build",
        ),
        (
            PlatformFailure::PermissionDenied,
            ErrorCategory::PermissionDenied,
            "permission_denied",
        ),
        (
            PlatformFailure::NoDefaultEndpoint,
            ErrorCategory::Unavailable,
            "no_default_endpoint",
        ),
        (
            PlatformFailure::NoMediaSession,
            ErrorCategory::Unavailable,
            "no_media_session",
        ),
        (
            PlatformFailure::StaleWindowReference,
            ErrorCategory::Unavailable,
            "stale_window_reference",
        ),
        (
            PlatformFailure::ForegroundDenied,
            ErrorCategory::PermissionDenied,
            "foreground_denied",
        ),
        (
            PlatformFailure::LaunchRejected,
            ErrorCategory::PermanentExternal,
            "launch_rejected",
        ),
        (
            PlatformFailure::OsApiUnavailable,
            ErrorCategory::Unavailable,
            "os_api_unavailable",
        ),
        (
            PlatformFailure::TransientRebind,
            ErrorCategory::TransientExternal,
            "transient_rebind",
        ),
    ];
    for (failure, category, code) in cases {
        let wire = failure.structured();
        assert_eq!(wire.category, category as i32);
        assert_eq!(wire.code, code);
        assert_eq!(wire.message_key, format!("platform.windows.error.{code}"));
    }
}
