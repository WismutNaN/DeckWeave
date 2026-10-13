// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{ScopedAuthorizationStore, permissions::PermissionCatalogSource};
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{PrincipalId, RuntimeGroupId},
};
use deckweave_platform_host::NativeChildProcessPolicy;
use deckweave_plugin_supervisor::{
    package::{IndexedPackage, PackageIndex, PackageStager, StagingLimits},
    process::{ProcessConfig, ProcessRuntime},
    supervisor::{RestartPolicy, RuntimeController, RuntimeLeaseReason, SupervisorAction},
};
use deckweave_protocol::{HostProtocolSupport, negotiate_hello, v1::*};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

const PACKAGES: [(&str, &str); 5] = [
    ("audio", env!("CARGO_BIN_EXE_deckweave-windows-audio")),
    ("media", env!("CARGO_BIN_EXE_deckweave-windows-media")),
    ("window", env!("CARGO_BIN_EXE_deckweave-windows-window")),
    (
        "applications",
        env!("CARGO_BIN_EXE_deckweave-windows-applications"),
    ),
    ("keyboard", env!("CARGO_BIN_EXE_deckweave-windows-keyboard")),
];
fn stage(root: &Path, name: &str, binary: &str) -> Result<IndexedPackage, Box<dyn Error>> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::create_dir_all(root.join("bin"))?;
    fs::create_dir_all(root.join("schemas"))?;
    let mut manifest: Value = serde_json::from_slice(&fs::read(
        source.join(format!("package/{name}.template.json")),
    )?)?;
    let bytes = fs::read(binary)?;
    let artifact = &mut manifest["artifacts"][0];
    fs::write(
        root.join(artifact["entrypoint"].as_str().ok_or("path")?),
        &bytes,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            root.join(artifact["entrypoint"].as_str().ok_or("path")?),
            fs::Permissions::from_mode(0o700),
        )?;
    }
    artifact["os"] = std::env::consts::OS.into();
    artifact["architecture"] = std::env::consts::ARCH.into();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    artifact["sha256"] = hash.clone().into();
    for entry in manifest["org.deckweave.automationCatalog"]["endpoints"]
        .as_array_mut()
        .ok_or("endpoints")?
    {
        entry["runtimeRequirement"]["artifactSha256"] = hash.clone().into();
    }
    for schema in manifest["schemas"].as_array_mut().ok_or("schemas")? {
        let path = schema["path"].as_str().ok_or("schema path")?;
        let bytes = fs::read(source.join(path))?;
        fs::write(root.join(path), &bytes)?;
        schema["sha256"] = format!("{:x}", Sha256::digest(&bytes)).into();
    }
    fs::write(
        root.join("deckweave-plugin.json"),
        serde_json::to_vec(&manifest)?,
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
            (deckweave_protocol::FEATURE_ACTIVATION_COMMIT.into(), 1),
            (
                deckweave_protocol::FEATURE_INVOCATION_ENDPOINT_PATH.into(),
                1,
            ),
        ]),
        schemas: BTreeMap::new(),
        limits: ProtocolLimits {
            maximum_message_bytes: 256 * 1024,
            maximum_chunk_bytes: 64 * 1024,
            maximum_concurrent_jobs: 8,
            maximum_artifact_bytes: 1024 * 1024,
            maximum_events_per_second: 512,
        },
    }
}
fn envelope(id: u64, body: protocol_envelope::Body) -> ProtocolEnvelope {
    ProtocolEnvelope {
        wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
        session_generation: 1,
        message_id: id,
        trace: None,
        body: Some(body),
    }
}

#[test]
fn audio_permissions_do_not_authorize_windows_or_keyboard() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let mut index = PackageIndex::default();
    for (name, binary) in PACKAGES {
        index.insert(stage(&temp.path().join(name), name, binary)?);
    }
    assert_eq!(index.editor_packages()?.len(), 5);
    let principal = PrincipalId::try_new("test-user")?;
    let offers = index.permission_offers(&principal)?;
    let grants = ScopedAuthorizationStore::default();
    grants.replace(
        offers
            .iter()
            .filter(|o| o.request.package_id.as_str().ends_with(".audio"))
            .map(|o| o.request.clone())
            .collect(),
    )?;
    let mut seen = BTreeSet::new();
    for package in index.packages() {
        let (id, _) = package.runtime_groups.iter().next().ok_or("group")?;
        let name = id.as_str();
        let mut controller = RuntimeController::new(
            Arc::new(package.clone()),
            id.clone(),
            RestartPolicy::default(),
        )?;
        controller.acquire_lease(RuntimeLeaseReason::Binding, None)?;
        let action = controller.check_permissions(&principal, &grants);
        if name == "audio" {
            assert_eq!(action?, SupervisorAction::StartProcess);
        } else {
            assert!(action.is_err() || !matches!(action, Ok(SupervisorAction::StartProcess)));
        }
        for scope in &package.requested_permissions {
            assert!(seen.insert(scope.scope.clone()));
        }
    }
    Ok(())
}

#[test]
fn standalone_packages_start_only_their_adapters_and_reject_other_families()
-> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    for (name, binary) in PACKAGES {
        let package = stage(&temp.path().join(name), name, binary)?;
        let mut process = ProcessRuntime::spawn_indexed(
            &package,
            &RuntimeGroupId::try_new(name)?,
            &[3; 32],
            1,
            ProcessConfig::default(),
            &BTreeMap::new(),
            &NativeChildProcessPolicy,
        )?;
        let Some(protocol_envelope::Body::Hello(hello)) =
            process.receive(Duration::from_secs(5))?.body
        else {
            return Err("hello".into());
        };
        assert_eq!(
            hello.package_id,
            format!("org.deckweave.platform.windows.{name}")
        );
        let ack = negotiate_hello(&hello, &support(), format!("session-{name}"))?;
        process.send(&envelope(1, protocol_envelope::Body::HelloAck(ack)))?;
        let contribution = package
            .contributions
            .keys()
            .next()
            .ok_or("contribution")?
            .to_string();
        process.send(&envelope(
            2,
            protocol_envelope::Body::Activate(ActivationRequest {
                runtime_lease_id: "lease".into(),
                contribution_id: contribution,
                settings: None,
                locale: None,
                configured_instance_id: None,
                settings_revision: 0,
                credential_refs: Default::default(),
            }),
        ))?;
        let Some(protocol_envelope::Body::Activated(reply)) =
            process.receive(Duration::from_secs(5))?.body
        else {
            return Err("activated".into());
        };
        let nodes: Vec<NodeDescriptor> =
            serde_json::from_slice(&reply.descriptor.ok_or("descriptor")?.value)?;
        for node in &nodes {
            node.validate()?;
        }
        let descriptor = serde_json::to_value(&nodes)?;
        if name == "keyboard" {
            assert!(descriptor[0]["devices"].is_array());
        } else {
            let elements = descriptor[0]["services"][0]["elements"]
                .as_array()
                .ok_or("elements")?;
            assert_eq!(elements.len(), 1);
            assert_eq!(
                elements[0]["elementId"],
                match name {
                    "audio" => "audio",
                    "media" => "media",
                    _ => "applications",
                }
            );
            let caps = elements[0]["capabilities"].as_array().ok_or("caps")?;
            if name == "window" || name == "applications" {
                for cap in caps {
                    assert_eq!(
                        matches!(cap["capabilityId"].as_str(), Some("launch" | "shell-open")),
                        name == "applications"
                    );
                }
            }
        }
        process.send(&envelope(
            3,
            protocol_envelope::Body::ActivationCommitted(ActivationCommitted {
                activation_epoch: 1,
                host_ticks: 0,
            }),
        ))?;
        // Cross-family probe uses no valid effect payload, even if admission regresses.
        let (element, capability) = if name == "audio" {
            ("media", "transport-control")
        } else {
            ("audio", "endpoint-control")
        };
        process.send(&envelope(
            4,
            protocol_envelope::Body::Invoke(InvocationRequest {
                configured_instance_id: None,
                invocation_id: "wrong-family".into(),
                run_id: "run".into(),
                operation_id: "probe".into(),
                activation_epoch: 1,
                descriptor_revision: 1,
                deadline_host_ticks: 1000,
                idempotency_key: None,
                input: None,
                target: Some(EndpointPath {
                    node_id: "windows-control-node".into(),
                    provider_id: "org.deckweave.platform.windows.control".into(),
                    target: Some(endpoint_path::Target::ServiceId("windows-platform".into())),
                    element_id: element.into(),
                    capability_id: capability.into(),
                    port_id: "invoke".into(),
                }),
                remaining_budget_ms: Some(1000),
            }),
        ))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let frame = process.receive(
                deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("reply deadline")?,
            )?;
            if let Some(protocol_envelope::Body::Signal(signal)) = &frame.body {
                let schema = signal
                    .payload
                    .as_ref()
                    .and_then(|p| p.schema.as_ref())
                    .ok_or("state schema")?;
                match name {
                    "audio" => assert_eq!(schema.id, "org.deckweave.windows.audio_state"),
                    "window" => assert_eq!(schema.id, "org.deckweave.windows.active_window_state"),
                    "keyboard" => {}
                    _ => return Err("unrelated OS observer was started".into()),
                }
            }
            if let Some(protocol_envelope::Body::InvocationReply(reply)) = frame.body {
                assert_eq!(reply.invocation_id, "wrong-family");
                assert!(matches!(
                    reply.reply,
                    Some(invocation_reply::Reply::Rejected(_))
                ));
                break;
            }
        }
        process.send(&envelope(
            5,
            protocol_envelope::Body::Deactivate(DeactivationRequest {
                runtime_lease_id: "lease".into(),
                reason: "test".into(),
            }),
        ))?;
        assert!(
            process
                .wait_timeout(Duration::from_secs(3))?
                .ok_or("stop timeout")?
                .success()
        );
    }
    Ok(())
}
