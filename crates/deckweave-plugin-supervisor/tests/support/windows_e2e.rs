// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in native process composition; no foreground, input or audio mutation.
use super::*;
use deckweave_automation::{ActionArguments, ValueBinding};
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    WindowQuery,
    WindowState,
    AudioState,
}

#[test]
#[ignore = "Run npm run test:windows-integration to build and stage real packages"]
fn virtual_button_reads_windows_catalog_and_delivers_typed_feedback() -> Result<(), Box<dyn Error>>
{
    run(Case::WindowQuery)
}
#[test]
#[ignore = "Run npm run test:windows-integration to build and stage real packages"]
fn native_audio_state_reaches_virtual_feedback_without_audio_mutation() -> Result<(), Box<dyn Error>>
{
    run(Case::AudioState)
}
#[test]
#[ignore = "Run npm run test:windows-integration to build and stage real packages"]
fn native_window_state_reaches_virtual_feedback_without_focus_mutation()
-> Result<(), Box<dyn Error>> {
    run(Case::WindowState)
}
fn run(case: Case) -> Result<(), Box<dyn Error>> {
    let bundles = PathBuf::from(std::env::var("DECKWEAVE_WINDOWS_E2E_BUNDLES")?);
    let family = if case == Case::AudioState {
        "audio"
    } else {
        "window"
    };
    let bundle = bundles.join(format!("windows-{family}"));
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let (mut draft, initial, feedback_marker) = m1_fixture::stage_and_build(root)?;
    let target = root.join(format!("packages/windows-{family}/content"));
    copy_tree(&bundle, &target)?;
    let mut index = PackageIndex::default();
    index.insert(
        deckweave_plugin_supervisor::package::PackageStager::inspect(
            &target,
            &StagingLimits::default(),
        )?
        .package,
    );
    let windows = index.descriptor_catalog()?;
    let query = windows
        .endpoints()
        .iter()
        .find(|e| match case {
            Case::WindowQuery => e
                .operation
                .as_ref()
                .is_some_and(|o| o.operation_id.as_str() == "list-windows"),
            Case::WindowState => {
                e.operation.is_none()
                    && e.port.schema.id.as_str() == "org.deckweave.windows.active_window_state"
            }
            Case::AudioState => {
                e.operation.is_none()
                    && e.port.schema.id.as_str() == "org.deckweave.windows.audio_state"
            }
        })
        .ok_or("Windows endpoint")?
        .clone();
    let result_schema = query
        .operation
        .as_ref()
        .map(|o| o.result_schema.clone())
        .unwrap_or_else(|| query.port.schema.clone());
    let result_contract = windows
        .schema(&result_schema)
        .ok_or("result contract")?
        .clone();
    let mut schemas = initial.catalog().schemas().to_vec();
    for schema in windows.schemas() {
        if !schemas.iter().any(|s| s.schema == schema.schema) {
            schemas.push(schema.clone());
        }
    }
    let mut endpoints = initial
        .catalog()
        .endpoints()
        .iter()
        .filter(|e| e.operation.is_none())
        .cloned()
        .collect::<Vec<_>>();
    let feedback = endpoints
        .iter_mut()
        .find(|e| e.endpoint.package_id.as_str() == m1_fixture::FEEDBACK_PACKAGE)
        .ok_or("feedback")?;
    feedback.port.schema = result_schema.clone();
    feedback.port.compatible_versions.minimum = result_schema.version;
    feedback.port.compatible_versions.maximum = result_schema.version;
    let schema_file = feedback_marker
        .parent()
        .ok_or("feedback parent")?
        .join("bin/feedback-schema");
    fs::write(schema_file, result_schema.id.as_str())?;
    let mut graph: GraphDocument = serde_json::from_value(draft.document)?;
    graph.schema_version.minor = 1;
    for node in &mut graph.graph.nodes {
        match node.id.as_str() {
            "action" if case == Case::WindowQuery => {
                let operation = query.operation.as_ref().ok_or("operation")?;
                node.kind = GraphNodeKind::Action {
                    endpoint: query.endpoint.clone(),
                    operation_id: operation.operation_id.clone(),
                    availability: EndpointAvailabilityPolicy::Required,
                };
                node.outputs[0].schema = result_schema.clone();
                node.outputs[1].schema = operation.error_schema.clone();
            }
            "result-state" | "feedback" => {
                for port in node.inputs.iter_mut().chain(node.outputs.iter_mut()) {
                    port.schema = result_schema.clone();
                }
                if let GraphNodeKind::State { initial_value, .. } = &mut node.kind {
                    *initial_value = None;
                }
            }
            _ => {}
        }
    }
    if case == Case::WindowQuery {
        graph.graph.action_arguments.insert(
            GraphNodeId::try_new("action")?,
            ActionArguments {
                trigger_port: GraphPortId::try_new("trigger")?,
                fields: BTreeMap::new(),
                value: Some(ValueBinding::Constant {
                    value: json!({"offset":0,"limit":10}),
                }),
            },
        );
    } else {
        graph
            .graph
            .nodes
            .retain(|n| ["state-source", "result-state", "feedback"].contains(&n.id.as_str()));
        let source = graph
            .graph
            .nodes
            .iter_mut()
            .find(|n| n.id.as_str() == "state-source")
            .ok_or("source")?;
        source.kind = GraphNodeKind::Source {
            endpoint: query.endpoint.clone(),
            availability: EndpointAvailabilityPolicy::Required,
        };
        source.outputs[0].schema = result_schema.clone();
        graph
            .graph
            .edges
            .retain(|e| ["action-state", "state-feedback"].contains(&e.id.as_str()));
        let edge = graph
            .graph
            .edges
            .iter_mut()
            .find(|e| e.id.as_str() == "action-state")
            .ok_or("edge")?;
        edge.source.node_id = GraphNodeId::try_new("state-source")?;
        edge.source.port_id = GraphPortId::try_new("value")?;
    }
    draft.document = serde_json::to_value(graph)?;
    draft.package_lock.push(PackageLockEntry {
        package_id: query.runtime_requirement.package_id.clone(),
        version: query.runtime_requirement.package_version.clone(),
        artifact_sha256: query.runtime_requirement.artifact_sha256.clone(),
        source: None,
        extensions: BTreeMap::new(),
    });
    endpoints.push(query);
    for package in ["m1-source", "m1-feedback"] {
        let path = root.join(format!(
            "packages/{package}/0.1.0/content/deckweave-plugin.json"
        ));
        let mut manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let owned = endpoints
            .iter()
            .filter(|e| {
                e.endpoint.package_id.as_str() == manifest["packageId"].as_str().unwrap_or_default()
            })
            .collect::<Vec<_>>();
        manifest["org.deckweave.automationCatalog"] =
            json!({"formatVersion":1,"schemas":schemas,"endpoints":owned});
        fs::write(path, serde_json::to_vec(&manifest)?)?;
    }
    let repository = Arc::new(JsonProfileStore::new(
        root.join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    repository.create_draft(&draft)?;
    let authorization = Arc::new(MutableAuthorization::new());
    let compiler = deckweave_plugin_supervisor::package::PackageGraphCompiler::new(Arc::new(
        PackageIndex::scan(&root.join("packages"), &StagingLimits::default())?,
    ))
    .with_authorization(
        Arc::clone(&authorization) as Arc<dyn AuthorizationStore>,
        PrincipalId::try_new("desktop-user")?,
    );
    let (runtime, apply, registry) = build_runtime_with_compiler(
        root,
        Arc::clone(&repository),
        Arc::clone(&authorization),
        "windows-e2e",
        Arc::new(compiler),
    )?;
    let (gateway, context) = gateway(root, Arc::clone(&repository), apply, Arc::clone(&runtime))?;
    let window = gateway.open_mutation_window(&context)?;
    authorization.revoke();
    assert!(
        gateway
            .query(
                &context,
                AppQuery::PlanApply {
                    draft_id: draft.draft_id.clone(),
                    expected_draft_version: draft.version,
                    expected_active: None,
                    allow_disruption: false
                }
            )
            .is_err()
    );
    assert!(
        registry
            .lock()
            .map_err(|_| "registry")?
            .snapshot()?
            .nodes
            .is_empty()
    );
    authorization.allowed.store(true, Ordering::SeqCst);
    authorization.revision.fetch_add(1, Ordering::SeqCst);
    let plan_hash = match gateway.query(
        &context,
        AppQuery::PlanApply {
            draft_id: draft.draft_id.clone(),
            expected_draft_version: draft.version,
            expected_active: None,
            allow_disruption: false,
        },
    )? {
        AppQueryResult::ApplyPlan(plan) => plan.plan_hash,
        _ => return Err("plan response".into()),
    };
    let operation_id = match gateway.command(
        &context,
        CommandEnvelope {
            request_id: "windows-e2e".into(),
            idempotency_key: IdempotencyKey::try_new("windows-e2e")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: draft.draft_id.clone(),
                expected_draft_version: draft.version,
                expected_active: None,
                allow_disruption: false,
                plan_hash,
            },
        },
    )? {
        AppCommandResult::OperationAccepted(receipt) => receipt.operation_id,
        _ => return Err("apply response".into()),
    };
    let expected_results = if case == Case::WindowQuery { 2 } else { 1 };
    let mut result = Vec::new();
    wait_until(|| {
        if let Ok(text) = fs::read_to_string(&feedback_marker) {
            result = text
                .lines()
                .take(expected_results)
                .map(serde_json::from_str::<Value>)
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default();
        }
        result.len() == expected_results
    })?;
    for value in result {
        assert!(result_contract.accepts(&value));
        if case == Case::WindowQuery {
            assert!(value["total"].as_u64().is_some());
            assert!(value["windows"].as_array().is_some_and(|v| v.len() <= 10));
        }
    }
    let scheduler = runtime.live_execution().snapshot()?.ok_or("scheduler")?;
    assert!(scheduler.pending.is_empty());
    assert_eq!(
        scheduler.completed.len(),
        if case == Case::WindowQuery { 2 } else { 0 }
    );
    assert_eq!(
        runtime
            .runtime_health()
            .runtimes
            .iter()
            .filter(|r| r.status == RuntimeHealthStatus::Active)
            .count(),
        if case == Case::WindowQuery { 3 } else { 2 }
    );
    let before = fs::read(&feedback_marker)?;
    authorization.revoke();
    wait_until(|| {
        registry
            .lock()
            .ok()
            .and_then(|r| r.snapshot().ok())
            .is_some_and(|s| s.nodes.is_empty())
    })?;
    runtime.shutdown()?;
    if case == Case::WindowQuery {
        assert_eq!(fs::read(&feedback_marker)?, before);
    }
    assert!(runtime.live_execution().snapshot()?.is_none());
    assert_eq!(
        repository.operation(&operation_id)?.state,
        ApplyOperationState::Succeeded
    );
    Ok(())
}
fn copy_tree(source: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(target)?;
    for item in fs::read_dir(source)? {
        let item = item?;
        let kind = item.file_type()?;
        if kind.is_dir() {
            copy_tree(&item.path(), &target.join(item.file_name()))?;
        } else if kind.is_file() {
            fs::copy(item.path(), target.join(item.file_name()))?;
        } else {
            return Err("unsupported bundle entry".into());
        }
    }
    Ok(())
}
