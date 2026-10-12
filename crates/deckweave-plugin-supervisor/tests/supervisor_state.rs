// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{AuthorizationError, AuthorizationStore};
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        ArtifactId, ContributionId, DescriptorRevision, GrantId, GrantRevision, PackageId,
        PluginSessionId, PrincipalId, RuntimeGroupId, RuntimeLeaseId,
    },
    security::{PermissionAccessMode, PermissionGrant, PermissionRequest},
};
use deckweave_plugin_supervisor::{
    package::{
        IndexedArtifact, IndexedContribution, IndexedPackage, IndexedRuntimeGroup,
        RequestedPermission,
    },
    supervisor::{
        RestartPolicy, RuntimeController, RuntimeLeaseReason, RuntimeStatus, SupervisorAction,
        SupervisorError,
    },
};
use deckweave_protocol::{
    HostProtocolSupport,
    v1::{
        FeatureRequirement, Hello, ProtocolEnvelope, ProtocolLimits, ProtocolVersion,
        ProtocolVersionRange,
    },
};
use deckweave_registry::{CapabilityRegistry, RegistryError};
use semver::Version;
use std::{collections::BTreeMap, error::Error, fs, path::PathBuf, sync::Arc};

struct AuthorizationFixture {
    denied_scope: Option<String>,
}

impl AuthorizationStore for AuthorizationFixture {
    fn revision(&self) -> Result<GrantRevision, AuthorizationError> {
        GrantRevision::try_new(1)
            .map_err(|error| AuthorizationError::Unavailable(error.to_string()))
    }

    fn authorize(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionGrant, AuthorizationError> {
        if self.denied_scope.as_deref() == Some(request.scope.as_str()) {
            return Err(AuthorizationError::Denied);
        }
        Ok(PermissionGrant {
            grant_id: GrantId::try_new(format!("grant-{}", request.scope.as_str()))
                .map_err(|error| AuthorizationError::Unavailable(error.to_string()))?,
            revision: self.revision()?,
            request: request.clone(),
            expires_at_rfc3339: None,
            extensions: Default::default(),
        })
    }
}

fn indexed_package() -> Result<Arc<IndexedPackage>, Box<dyn Error>> {
    let artifact_id = ArtifactId::try_new("current")?;
    let control = RuntimeGroupId::try_new("control")?;
    let optional = RuntimeGroupId::try_new("optional")?;
    let artifact = IndexedArtifact {
        artifact_id: artifact_id.clone(),
        operating_system: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        entrypoint: PathBuf::from("virtual-plugin"),
        sha256: "a".repeat(64),
    };
    let group = |id: RuntimeGroupId| IndexedRuntimeGroup {
        runtime_group_id: id,
        runtime_class: "isolated_control".into(),
        artifact_id: artifact_id.clone(),
    };
    Ok(Arc::new(IndexedPackage {
        package_id: PackageId::try_new("org.deckweave.fixture.virtual")?,
        version: Version::parse("0.1.0")?,
        package_root: PathBuf::from("virtual-package"),
        manifest_sha256: "b".repeat(64),
        artifacts: BTreeMap::from([(artifact_id.clone(), artifact)]),
        runtime_groups: BTreeMap::from([
            (control.clone(), group(control.clone())),
            (optional.clone(), group(optional.clone())),
        ]),
        contributions: BTreeMap::from([(
            ContributionId::try_new("virtual-device")?,
            IndexedContribution {
                contribution_id: ContributionId::try_new("virtual-device")?,
                contribution_type: deckweave_domain::ids::NamespacedKind::try_new(
                    "org.deckweave.device_provider",
                )?,
                runtime_group_id: control.clone(),
            },
        )]),
        requested_permissions: vec![
            RequestedPermission {
                scope: deckweave_domain::ids::NamespacedKind::try_new(
                    "org.deckweave.device.virtual",
                )?,
                access_mode: PermissionAccessMode::Execute,
                resource_selector: serde_json::json!({ "kind": "virtual" }),
                reason: "Virtual fixture".into(),
                optional: false,
                runtime_group_ids: vec![control],
            },
            RequestedPermission {
                scope: deckweave_domain::ids::NamespacedKind::try_new(
                    "org.deckweave.network.optional",
                )?,
                access_mode: PermissionAccessMode::Execute,
                resource_selector: serde_json::json!({ "domain": "example.invalid" }),
                reason: "Optional integration".into(),
                optional: false,
                runtime_group_ids: vec![optional],
            },
        ],
    }))
}

fn support() -> HostProtocolSupport {
    HostProtocolSupport {
        protocol: ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        },
        features: BTreeMap::from([("org.deckweave.jobs".into(), 1)]),
        schemas: BTreeMap::new(),
        limits: limits(1024),
    }
}

fn limits(value: u64) -> ProtocolLimits {
    ProtocolLimits {
        maximum_message_bytes: value,
        maximum_chunk_bytes: value,
        maximum_concurrent_jobs: 4,
        maximum_artifact_bytes: value,
        maximum_events_per_second: 100,
    }
}

fn hello(token: Vec<u8>) -> Hello {
    Hello {
        protocol: Some(ProtocolVersionRange {
            minimum: Some(ProtocolVersion { major: 1, minor: 0 }),
            maximum: Some(ProtocolVersion { major: 1, minor: 1 }),
        }),
        package_id: "org.deckweave.fixture.virtual".into(),
        runtime_group: "control".into(),
        one_time_token: token,
        features: vec![],
        schemas: vec![],
        limits: Some(limits(2048)),
    }
}

fn observe_node() -> Result<NodeDescriptor, Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/fixtures/descriptors/observe-keyboard.node.json");
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn activate(
    controller: &mut RuntimeController,
    registry: &mut CapabilityRegistry,
    now: u64,
) -> Result<RuntimeLeaseId, Box<dyn Error>> {
    let (lease, action) = controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    assert_eq!(action, SupervisorAction::CheckPermissions);
    assert_eq!(
        controller.check_permissions(
            &PrincipalId::try_new("local-user")?,
            &AuthorizationFixture { denied_scope: None }
        )?,
        SupervisorAction::StartProcess
    );
    let token = vec![7; 32];
    controller.process_started(
        PluginSessionId::try_new(format!("session-{now}"))?,
        token.clone(),
        now,
    )?;
    let ack = controller.accept_hello(&hello(token), &support())?;
    assert_eq!(
        ack.limits.map(|value| value.maximum_message_bytes),
        Some(1024)
    );
    controller.activate_descriptors(
        registry,
        DescriptorRevision::try_new(1)?,
        vec![observe_node()?],
        now,
    )?;
    Ok(lease.lease_id)
}

#[test]
fn wire_version_cannot_refresh_liveness_before_or_outside_negotiation() -> Result<(), Box<dyn Error>>
{
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut registry = CapabilityRegistry::default();
    activate(&mut controller, &mut registry, 100)?;
    let generation = controller
        .session()
        .ok_or("session")?
        .identity
        .generation
        .get();
    for wire_version in [
        None,
        Some(ProtocolVersion { major: 2, minor: 0 }),
        Some(ProtocolVersion {
            major: 1,
            minor: 99,
        }),
    ] {
        let message = ProtocolEnvelope {
            wire_version,
            session_generation: generation,
            ..Default::default()
        };
        assert!(matches!(
            controller.accept_envelope(&message, 500),
            Err(SupervisorError::InvalidWireVersion)
        ));
        assert_eq!(
            controller
                .session()
                .ok_or("session")?
                .last_heartbeat_host_ticks,
            100
        );
    }
    let mut message = ProtocolEnvelope {
        wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
        session_generation: generation,
        ..Default::default()
    };
    controller.accept_envelope(&message, 501)?;
    assert_eq!(
        controller
            .session()
            .ok_or("session")?
            .last_heartbeat_host_ticks,
        501
    );
    // Starting a replacement does not inherit the previous handshake.
    controller.process_exited(&mut registry, 502, false)?;
    assert_eq!(controller.tick(1000)?, SupervisorAction::CheckPermissions);
    controller.check_permissions(
        &PrincipalId::try_new("local-user")?,
        &AuthorizationFixture { denied_scope: None },
    )?;
    controller.process_started(PluginSessionId::try_new("next")?, vec![4; 32], 1000)?;
    message.session_generation = controller
        .session()
        .ok_or("session")?
        .identity
        .generation
        .get();
    assert!(matches!(
        controller.accept_envelope(&message, 1001),
        Err(SupervisorError::InvalidWireVersion)
    ));
    Ok(())
}

#[test]
fn lease_drives_permission_handshake_activation_and_idle_stop() -> Result<(), Box<dyn Error>> {
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut registry = CapabilityRegistry::default();
    let lease_id = activate(&mut controller, &mut registry, 100)?;
    assert_eq!(controller.status(), RuntimeStatus::Active);
    assert_eq!(registry.snapshot()?.nodes.len(), 1);

    controller.release_lease(&lease_id, 200)?;
    assert_eq!(controller.tick(2_199)?, SupervisorAction::None);
    assert_eq!(
        controller.tick(2_200)?,
        SupervisorAction::RequestGracefulStop
    );
    controller.process_exited(&mut registry, 2_201, true)?;
    assert_eq!(controller.status(), RuntimeStatus::Indexed);
    assert!(registry.snapshot()?.nodes.is_empty());
    Ok(())
}

#[test]
fn invalid_initial_descriptors_never_publish_an_empty_session() -> Result<(), Box<dyn Error>> {
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut registry = CapabilityRegistry::default();
    controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    controller.check_permissions(
        &PrincipalId::try_new("local-user")?,
        &AuthorizationFixture { denied_scope: None },
    )?;
    let token = vec![4; 32];
    controller.process_started(
        PluginSessionId::try_new("invalid-descriptors")?,
        token.clone(),
        1,
    )?;
    controller.accept_hello(&hello(token), &support())?;
    let node = observe_node()?;

    assert!(matches!(
        controller.activate_descriptors(
            &mut registry,
            DescriptorRevision::try_new(1)?,
            vec![node.clone(), node],
            1,
        ),
        Err(SupervisorError::Registry(RegistryError::DuplicateNode(_)))
    ));
    assert_eq!(controller.status(), RuntimeStatus::Negotiating);
    assert!(registry.snapshot()?.nodes.is_empty());

    controller.process_exited(&mut registry, 2, false)?;
    assert!(registry.snapshot()?.nodes.is_empty());
    Ok(())
}

#[test]
fn permissions_are_scoped_to_runtime_group() -> Result<(), Box<dyn Error>> {
    let package = indexed_package()?;
    let denied = AuthorizationFixture {
        denied_scope: Some("org.deckweave.network.optional".into()),
    };
    let mut control = RuntimeController::new(
        package.clone(),
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    control.acquire_lease(
        RuntimeLeaseReason::Discovery {
            scope: "virtual".into(),
        },
        None,
    )?;
    assert_eq!(
        control.check_permissions(&PrincipalId::try_new("local-user")?, &denied)?,
        SupervisorAction::StartProcess
    );

    let mut optional = RuntimeController::new(
        package,
        RuntimeGroupId::try_new("optional")?,
        RestartPolicy::default(),
    )?;
    optional.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    assert!(matches!(
        optional.check_permissions(&PrincipalId::try_new("local-user")?, &denied),
        Err(SupervisorError::Permission(_))
    ));
    assert_eq!(optional.status(), RuntimeStatus::NeedsAttention);
    Ok(())
}

#[test]
fn unsupported_required_feature_blocks_handshake() -> Result<(), Box<dyn Error>> {
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    controller.check_permissions(
        &PrincipalId::try_new("local-user")?,
        &AuthorizationFixture { denied_scope: None },
    )?;
    let token = vec![9; 32];
    controller.process_started(
        PluginSessionId::try_new("session-features")?,
        token.clone(),
        1,
    )?;
    let mut incompatible = hello(token);
    incompatible.features.push(FeatureRequirement {
        id: "org.deckweave.future.required".into(),
        minimum_version: 1,
        maximum_version: 1,
        required: true,
    });
    assert!(matches!(
        controller.accept_hello(&incompatible, &support()),
        Err(SupervisorError::Negotiation(_))
    ));
    assert_eq!(controller.status(), RuntimeStatus::Negotiating);
    Ok(())
}

#[test]
fn crash_backoff_opens_circuit_and_old_generation_is_rejected() -> Result<(), Box<dyn Error>> {
    let policy = RestartPolicy {
        maximum_crashes: 3,
        base_backoff_ticks: 10,
        maximum_backoff_ticks: 100,
        ..RestartPolicy::default()
    };
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        policy,
    )?;
    let mut registry = CapabilityRegistry::default();
    let _lease = activate(&mut controller, &mut registry, 1)?;
    let old_generation = controller
        .session()
        .ok_or("session missing")?
        .identity
        .generation
        .get();

    controller.process_exited(&mut registry, 10, false)?;
    assert_eq!(controller.status(), RuntimeStatus::Backoff);
    assert_eq!(controller.tick(20)?, SupervisorAction::CheckPermissions);
    controller.check_permissions(
        &PrincipalId::try_new("local-user")?,
        &AuthorizationFixture { denied_scope: None },
    )?;
    controller.process_started(PluginSessionId::try_new("session-2")?, vec![2; 32], 20)?;
    let late = ProtocolEnvelope {
        session_generation: old_generation,
        ..ProtocolEnvelope::default()
    };
    assert!(matches!(
        controller.accept_envelope(&late, 21),
        Err(SupervisorError::LateSessionMessage)
    ));

    controller.process_exited(&mut registry, 30, false)?;
    assert_eq!(controller.tick(50)?, SupervisorAction::CheckPermissions);
    controller.check_permissions(
        &PrincipalId::try_new("local-user")?,
        &AuthorizationFixture { denied_scope: None },
    )?;
    controller.process_started(PluginSessionId::try_new("session-3")?, vec![3; 32], 50)?;
    controller.process_exited(&mut registry, 60, false)?;
    assert_eq!(controller.status(), RuntimeStatus::CircuitOpen);
    assert_eq!(controller.health().crash_count_in_window, 3);
    Ok(())
}

#[test]
fn expiring_discovery_lease_stops_after_grace() -> Result<(), Box<dyn Error>> {
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut registry = CapabilityRegistry::default();
    let (lease, action) = controller.acquire_lease(
        RuntimeLeaseReason::Discovery {
            scope: "org.deckweave.device.virtual".into(),
        },
        Some(500),
    )?;
    assert_eq!(action, SupervisorAction::CheckPermissions);
    controller.check_permissions(
        &PrincipalId::try_new("local-user")?,
        &AuthorizationFixture { denied_scope: None },
    )?;
    let token = vec![5; 32];
    controller.process_started(
        PluginSessionId::try_new("discovery-session")?,
        token.clone(),
        1,
    )?;
    controller.accept_hello(&hello(token), &support())?;
    controller.activate_descriptors(
        &mut registry,
        DescriptorRevision::try_new(1)?,
        vec![observe_node()?],
        1,
    )?;

    assert_eq!(controller.tick(500)?, SupervisorAction::None);
    assert_eq!(controller.health().lease_count, 0);
    assert_eq!(controller.tick(2_499)?, SupervisorAction::None);
    assert_eq!(
        controller.tick(2_500)?,
        SupervisorAction::RequestGracefulStop
    );
    assert!(matches!(
        controller.release_lease(&lease.lease_id, 2_500),
        Err(SupervisorError::UnknownLease)
    ));
    Ok(())
}

#[test]
fn lease_arriving_during_stop_restarts_after_expected_exit() -> Result<(), Box<dyn Error>> {
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut registry = CapabilityRegistry::default();
    let first = activate(&mut controller, &mut registry, 1)?;
    controller.release_lease(&first, 10)?;
    assert_eq!(
        controller.tick(2_010)?,
        SupervisorAction::RequestGracefulStop
    );
    let (_replacement, action) = controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    assert_eq!(action, SupervisorAction::None);
    assert_eq!(
        controller.process_exited(&mut registry, 2_011, true)?,
        SupervisorAction::CheckPermissions
    );
    assert_eq!(controller.status(), RuntimeStatus::PermissionCheck);
    Ok(())
}

#[test]
fn revoked_grant_removes_routes_and_blocks_restart() -> Result<(), Box<dyn Error>> {
    let mut controller = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut registry = CapabilityRegistry::default();
    let _lease = activate(&mut controller, &mut registry, 1)?;
    assert_eq!(registry.snapshot()?.nodes.len(), 1);

    assert_eq!(
        controller.revalidate_permissions(
            &PrincipalId::try_new("local-user")?,
            &AuthorizationFixture {
                denied_scope: Some("org.deckweave.device.virtual".into()),
            },
            &mut registry,
            10,
        )?,
        SupervisorAction::RequestGracefulStop
    );
    assert!(registry.snapshot()?.nodes.is_empty());
    controller.process_exited(&mut registry, 11, true)?;
    assert_eq!(controller.status(), RuntimeStatus::NeedsAttention);
    assert_eq!(controller.tick(1_000_000)?, SupervisorAction::None);
    Ok(())
}

#[test]
fn startup_and_heartbeat_deadlines_force_bounded_cleanup() -> Result<(), Box<dyn Error>> {
    let mut startup = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    startup.acquire_lease(RuntimeLeaseReason::Binding, None)?;
    startup.check_permissions(
        &PrincipalId::try_new("local-user")?,
        &AuthorizationFixture { denied_scope: None },
    )?;
    startup.process_started(PluginSessionId::try_new("startup-timeout")?, vec![8; 32], 1)?;
    assert_eq!(startup.tick(10_001)?, SupervisorAction::ForceTerminate);
    assert_eq!(startup.status(), RuntimeStatus::Stopping);

    let mut heartbeat = RuntimeController::new(
        indexed_package()?,
        RuntimeGroupId::try_new("control")?,
        RestartPolicy::default(),
    )?;
    let mut registry = CapabilityRegistry::default();
    let _lease = activate(&mut heartbeat, &mut registry, 1)?;
    assert_eq!(heartbeat.tick(15_001)?, SupervisorAction::ForceTerminate);
    assert_eq!(heartbeat.status(), RuntimeStatus::Stopping);
    Ok(())
}
