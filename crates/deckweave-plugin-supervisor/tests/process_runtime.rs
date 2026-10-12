// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{AuthorizationError, AuthorizationStore};
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        ArtifactId, DescriptorRevision, GrantRevision, PackageId, PluginSessionId, PrincipalId,
        RuntimeGroupId,
    },
    security::{PermissionGrant, PermissionRequest},
};
use deckweave_platform_host::NativeChildProcessPolicy;
use deckweave_plugin_supervisor::{
    package::{IndexedArtifact, IndexedPackage, IndexedRuntimeGroup},
    process::{ProcessConfig, ProcessError, ProcessRuntime},
    supervisor::{
        RestartPolicy, RuntimeController, RuntimeLeaseReason, RuntimeStatus, SupervisorAction,
    },
};
use deckweave_protocol::{
    HostProtocolSupport,
    framing::FramingError,
    v1::{
        ActivationRequest, BackpressureNotice, DeactivationRequest, ProtocolEnvelope,
        ProtocolLimits, ProtocolVersion, protocol_envelope,
    },
};
use deckweave_registry::CapabilityRegistry;
use semver::Version;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::PathBuf,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

const FIXTURE: &str = env!("CARGO_BIN_EXE_deckweave-virtual-plugin-fixture");

fn indexed_fixture() -> Result<(IndexedPackage, RuntimeGroupId), Box<dyn Error>> {
    let runtime_group_id = RuntimeGroupId::try_new("control")?;
    let artifact_id = ArtifactId::try_new("native")?;
    let entrypoint = PathBuf::from(FIXTURE);
    let package_root = entrypoint
        .parent()
        .ok_or("fixture has no parent")?
        .to_path_buf();
    let artifact_sha256 = format!("{:x}", Sha256::digest(fs::read(&entrypoint)?));
    Ok((
        IndexedPackage {
            package_id: PackageId::try_new("org.deckweave.fixture.virtual")?,
            version: Version::parse("0.1.0")?,
            package_root,
            manifest_sha256: "0".repeat(64),
            artifacts: BTreeMap::from([(
                artifact_id.clone(),
                IndexedArtifact {
                    artifact_id: artifact_id.clone(),
                    operating_system: std::env::consts::OS.into(),
                    architecture: std::env::consts::ARCH.into(),
                    entrypoint,
                    sha256: artifact_sha256,
                },
            )]),
            runtime_groups: BTreeMap::from([(
                runtime_group_id.clone(),
                IndexedRuntimeGroup {
                    runtime_group_id: runtime_group_id.clone(),
                    runtime_class: "isolated_control".into(),
                    artifact_id,
                },
            )]),
            contributions: BTreeMap::new(),
            requested_permissions: vec![],
        },
        runtime_group_id,
    ))
}

fn spawn_with_environment(
    environment: BTreeMap<String, String>,
) -> Result<ProcessRuntime, Box<dyn Error>> {
    spawn_with_config(environment, fixture_process_config())
}

fn fixture_process_config() -> ProcessConfig {
    ProcessConfig {
        maximum_message_bytes: 64 * 1024,
        maximum_stderr_bytes: 1024,
        maximum_artifact_bytes: 16 * 1024 * 1024,
        ..ProcessConfig::default()
    }
}

fn spawn_with_config(
    environment: BTreeMap<String, String>,
    config: ProcessConfig,
) -> Result<ProcessRuntime, Box<dyn Error>> {
    let (package, runtime_group_id) = indexed_fixture()?;
    Ok(ProcessRuntime::spawn_indexed(
        &package,
        &runtime_group_id,
        &[7; 32],
        1,
        config,
        &environment,
        &NativeChildProcessPolicy,
    )?)
}

fn spawn(mode: Option<&str>) -> Result<ProcessRuntime, Box<dyn Error>> {
    spawn_with_environment(
        mode.map(|value| BTreeMap::from([("DECKWEAVE_FIXTURE_MODE".into(), value.into())]))
            .unwrap_or_default(),
    )
}

fn staged_package()
-> Result<(tempfile::TempDir, Arc<IndexedPackage>, RuntimeGroupId), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory
        .path()
        .join("org.deckweave.fixture.virtual/0.1.0/content");
    fs::create_dir_all(root.join("bin"))?;
    fs::create_dir_all(root.join("schema"))?;
    let artifact_name = format!("plugin{}", std::env::consts::EXE_SUFFIX);
    let artifact_path = root.join("bin").join(&artifact_name);
    fs::copy(FIXTURE, &artifact_path)?;
    let artifact_bytes = fs::read(&artifact_path)?;
    let schema = br#"{"type":"object","additionalProperties":{}}"#;
    fs::write(root.join("schema/settings.json"), schema)?;
    let manifest = serde_json::json!({
        "manifestVersion": 1,
        "packageId": "org.deckweave.fixture.virtual",
        "version": "0.1.0",
        "license": "MIT OR Apache-2.0",
        "source": {
            "repository": "https://example.invalid/deckweave/virtual",
            "revision": "process-runtime-fixture"
        },
        "artifacts": [{
            "id": "current-platform",
            "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "entrypoint": format!("bin/{artifact_name}"),
            "sha256": format!("{:x}", Sha256::digest(&artifact_bytes))
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
        "permissions": [],
        "schemas": [{
            "id": "org.deckweave.fixture.settings",
            "version": { "major": 1, "minor": 0 },
            "path": "schema/settings.json",
            "sha256": format!("{:x}", Sha256::digest(schema))
        }]
    });
    fs::write(
        root.join("deckweave-plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let staged = deckweave_plugin_supervisor::package::PackageStager::inspect(
        &root,
        &deckweave_plugin_supervisor::package::StagingLimits::default(),
    )?;
    let runtime_group = RuntimeGroupId::try_new("control")?;
    Ok((directory, Arc::new(staged.package), runtime_group))
}

struct AllowAll;

impl AuthorizationStore for AllowAll {
    fn revision(&self) -> Result<GrantRevision, AuthorizationError> {
        GrantRevision::try_new(1)
            .map_err(|error| AuthorizationError::Unavailable(error.to_string()))
    }

    fn authorize(
        &self,
        _request: &PermissionRequest,
    ) -> Result<PermissionGrant, AuthorizationError> {
        Err(AuthorizationError::Unavailable(
            "fixture declares no permissions".into(),
        ))
    }
}

fn host_support() -> HostProtocolSupport {
    HostProtocolSupport {
        protocol: deckweave_protocol::v1::ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        },
        features: BTreeMap::from([
            (
                deckweave_protocol::FEATURE_INVOCATION_ENDPOINT_PATH.into(),
                1,
            ),
            (
                deckweave_protocol::FEATURE_VALUE_WRITE_ENDPOINT_PATH.into(),
                1,
            ),
        ]),
        schemas: BTreeMap::new(),
        limits: ProtocolLimits {
            maximum_message_bytes: 1024,
            maximum_chunk_bytes: 1024,
            maximum_concurrent_jobs: 4,
            maximum_artifact_bytes: 64 * 1024,
            maximum_events_per_second: 100,
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

#[test]
fn isolated_process_completes_handshake_activation_and_stop() -> Result<(), Box<dyn Error>> {
    let (_staged_package, package, runtime_group) = staged_package()?;
    let mut controller = RuntimeController::new(
        Arc::clone(&package),
        runtime_group.clone(),
        RestartPolicy::default(),
    )?;
    let (lease, action) = controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    assert_eq!(action, SupervisorAction::CheckPermissions);
    assert_eq!(
        controller.check_permissions(&PrincipalId::try_new("local-user")?, &AllowAll)?,
        SupervisorAction::StartProcess
    );
    let token = vec![7; 32];
    let mut process = ProcessRuntime::spawn_indexed(
        &package,
        &runtime_group,
        &token,
        1,
        ProcessConfig {
            maximum_message_bytes: 64 * 1024,
            maximum_stderr_bytes: 1024,
            maximum_artifact_bytes: 16 * 1024 * 1024,
            ..ProcessConfig::default()
        },
        &BTreeMap::new(),
        &NativeChildProcessPolicy,
    )?;
    controller.process_started(PluginSessionId::try_new("session-1")?, token, 100)?;
    let hello = process.receive(Duration::from_secs(2))?;
    assert!(matches!(
        hello.body.as_ref(),
        Some(protocol_envelope::Body::Hello(value))
            if value.package_id == "org.deckweave.fixture.virtual"
                && value.runtime_group == "control"
                && value.one_time_token == [7; 32]
    ));
    let Some(protocol_envelope::Body::Hello(hello_message)) = hello.body else {
        return Err("expected hello".into());
    };
    let hello_ack = controller.accept_hello(&hello_message, &host_support())?;
    let negotiated_maximum = usize::try_from(
        hello_ack
            .limits
            .as_ref()
            .ok_or("negotiated limits missing")?
            .maximum_message_bytes,
    )?;
    assert_eq!(negotiated_maximum, 1024);
    process.constrain_maximum_message_bytes(negotiated_maximum)?;

    process.send(&envelope(1, protocol_envelope::Body::HelloAck(hello_ack)))?;
    process.send(&envelope(
        2,
        protocol_envelope::Body::Activate(ActivationRequest {
            runtime_lease_id: lease.lease_id.to_string(),
            contribution_id: "virtual-device".into(),
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
    assert_eq!(activated.runtime_lease_id, lease.lease_id.as_str());
    let descriptor = activated.descriptor.ok_or("descriptor missing")?;
    let nodes: Vec<NodeDescriptor> = serde_json::from_slice(&descriptor.value)?;
    assert_eq!(nodes.len(), 1);
    let mut registry = CapabilityRegistry::default();
    controller.activate_descriptors(
        &mut registry,
        DescriptorRevision::try_new(activated.descriptor_revision)?,
        nodes,
        110,
    )?;
    assert_eq!(controller.status(), RuntimeStatus::Active);
    assert_eq!(registry.snapshot()?.nodes.len(), 1);

    let heartbeat = process.receive(Duration::from_secs(2))?;
    assert!(matches!(
        heartbeat.body.as_ref(),
        Some(protocol_envelope::Body::Heartbeat(_))
    ));
    controller.accept_envelope(&heartbeat, 120)?;
    controller.release_lease(&lease.lease_id, 200)?;
    assert_eq!(controller.tick(2_199)?, SupervisorAction::None);
    assert_eq!(
        controller.tick(2_200)?,
        SupervisorAction::RequestGracefulStop
    );
    process.send(&envelope(
        3,
        protocol_envelope::Body::Deactivate(DeactivationRequest {
            runtime_lease_id: lease.lease_id.to_string(),
            reason: "idle".into(),
        }),
    ))?;
    let status = process
        .wait_timeout(Duration::from_secs(2))?
        .ok_or("fixture did not stop")?;
    assert!(status.success());
    controller.process_exited(&mut registry, 2_201, true)?;
    assert_eq!(controller.status(), RuntimeStatus::Indexed);
    assert!(registry.snapshot()?.nodes.is_empty());
    assert!(process.diagnostics().stderr.contains("fixture started"));
    Ok(())
}

#[test]
fn oversized_frame_is_rejected_without_reading_payload() -> Result<(), Box<dyn Error>> {
    let mut process = spawn(Some("oversized"))?;
    assert!(matches!(
        process.receive(Duration::from_secs(2)),
        Err(ProcessError::Framing(FramingError::TooLarge { .. }))
    ));
    let _status = process.wait_timeout(Duration::from_secs(2))?;
    Ok(())
}

#[test]
fn noisy_stderr_is_drained_but_diagnostics_stay_bounded() -> Result<(), Box<dyn Error>> {
    let mut process = spawn(Some("spam_stderr"))?;
    assert!(matches!(
        process.receive(Duration::from_secs(2))?.body,
        Some(protocol_envelope::Body::Hello(_))
    ));
    process.send(&envelope(
        1,
        protocol_envelope::Body::Deactivate(DeactivationRequest {
            runtime_lease_id: "diagnostics".into(),
            reason: "test".into(),
        }),
    ))?;
    let status = process
        .wait_timeout(Duration::from_secs(2))?
        .ok_or("noisy fixture did not stop")?;
    assert!(status.success());
    let diagnostics = process.diagnostics();
    assert_eq!(diagnostics.stderr.len(), 1024);
    assert!(diagnostics.truncated);
    Ok(())
}

#[test]
fn control_queue_overflow_is_explicit_and_never_unbounded() -> Result<(), Box<dyn Error>> {
    let mut process = spawn(Some("spam_stdout"))?;
    std::thread::sleep(Duration::from_millis(100));
    let mut observed_overflow = false;
    for _ in 0..32 {
        match process.receive(Duration::from_secs(2)) {
            Ok(_) => {}
            Err(ProcessError::QueueOverflow) => {
                observed_overflow = true;
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }
    assert!(observed_overflow);
    let _status = process.wait_timeout(Duration::from_secs(2))?;
    Ok(())
}

#[test]
fn unread_stdin_cannot_block_watchdog_or_grow_reliable_queue() -> Result<(), Box<dyn Error>> {
    let mut config = fixture_process_config();
    config.maximum_outbound_frames = 2;
    let mut process = spawn_with_config(
        BTreeMap::from([("DECKWEAVE_FIXTURE_MODE".into(), "ignore_stdin".into())]),
        config,
    )?;
    let _hello = process.receive(Duration::from_secs(2))?;
    let large = |message_id| {
        envelope(
            message_id,
            protocol_envelope::Body::Backpressure(BackpressureNotice {
                lane: "reliable-test".into(),
                dropped_or_coalesced: 0,
                policy: "x".repeat(60 * 1024),
            }),
        )
    };

    let started = Instant::now();
    let mut overflowed = false;
    for message_id in 1..=32 {
        match process.send(&large(message_id)) {
            Ok(()) => {}
            Err(ProcessError::OutboundQueueOverflow) => {
                overflowed = true;
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }
    assert!(overflowed, "bounded reliable queue never reported overflow");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches!(
        process.send(&large(100)),
        Err(ProcessError::OutboundQueueOverflow)
    ));
    assert!(process.wait_timeout(Duration::from_secs(2))?.is_some());
    Ok(())
}

#[test]
fn latest_value_lane_coalesces_without_filling_reliable_control() -> Result<(), Box<dyn Error>> {
    let mut config = fixture_process_config();
    config.maximum_outbound_frames = 2;
    config.maximum_coalesced_keys = 2;
    let mut process = spawn_with_config(
        BTreeMap::from([("DECKWEAVE_FIXTURE_MODE".into(), "ignore_stdin".into())]),
        config,
    )?;
    let _hello = process.receive(Duration::from_secs(2))?;
    for message_id in 1..=32 {
        process.send_latest(
            "surface/frame",
            &envelope(
                message_id,
                protocol_envelope::Body::Backpressure(BackpressureNotice {
                    lane: "latest-test".into(),
                    dropped_or_coalesced: message_id,
                    policy: "x".repeat(60 * 1024),
                }),
            ),
        )?;
    }
    assert!(process.diagnostics().coalesced_outbound > 0);
    let _status = process.terminate()?;
    Ok(())
}

#[test]
fn parent_exit_with_inherited_pipes_is_bounded_by_external_watchdog() -> Result<(), Box<dyn Error>>
{
    let mut process = spawn(Some("exit_with_inherited_pipes"))?;
    let _hello = process.receive(Duration::from_secs(2))?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = process
            .wait_timeout(Duration::from_secs(1))
            .map(|status| status.is_some())
            .map_err(|error| error.to_string());
        let _ignored = sender.send(result);
    });

    let result = receiver.recv_timeout(Duration::from_secs(3))?;
    assert_eq!(result, Ok(true));
    Ok(())
}

#[test]
fn crash_and_hung_shutdown_are_observable_and_bounded() -> Result<(), Box<dyn Error>> {
    let mut crashed = spawn(Some("crash_after_hello"))?;
    assert!(matches!(
        crashed.receive(Duration::from_secs(2))?.body,
        Some(protocol_envelope::Body::Hello(_))
    ));
    let crash_status = crashed
        .wait_timeout(Duration::from_secs(2))?
        .ok_or("crashing fixture did not exit")?;
    assert!(!crash_status.success());

    let mut hung = spawn(Some("hang_on_stop"))?;
    let _hello = hung.receive(Duration::from_secs(2))?;
    hung.send(&envelope(
        1,
        protocol_envelope::Body::Deactivate(DeactivationRequest {
            runtime_lease_id: "lease-1".into(),
            reason: "test stop deadline".into(),
        }),
    ))?;
    assert!(hung.wait_timeout(Duration::from_millis(100))?.is_none());
    assert!(!hung.terminate()?.success());
    Ok(())
}

#[test]
fn termination_cleans_up_the_plugin_process_tree() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("orphan-marker");
    let mut process = spawn_with_environment(BTreeMap::from([
        ("DECKWEAVE_FIXTURE_MODE".into(), "spawn_grandchild".into()),
        (
            "DECKWEAVE_FIXTURE_MARKER".into(),
            marker.to_string_lossy().into_owned(),
        ),
    ]))?;
    let _hello = process.receive(Duration::from_secs(2))?;
    process.send(&envelope(
        1,
        protocol_envelope::Body::Activate(ActivationRequest {
            runtime_lease_id: "lease-tree".into(),
            contribution_id: "virtual-device".into(),
            settings: None,
            locale: None,
            configured_instance_id: None,
            settings_revision: 0,
            credential_refs: Default::default(),
        }),
    ))?;
    let _activated = process.receive(Duration::from_secs(2))?;
    let _terminated = process.terminate()?;
    std::thread::sleep(Duration::from_millis(700));
    assert!(!marker.exists(), "plugin grandchild survived runtime stop");
    Ok(())
}
