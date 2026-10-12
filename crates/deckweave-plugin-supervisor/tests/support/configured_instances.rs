// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_domain::{ids::PluginInstanceId, localization::LocaleContext};
use deckweave_workspace::PluginInstanceConfiguration;
use serde_json::json;

#[test]
fn static_catalog_activates_two_accounts_in_one_process_and_recovers_settings()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let hash = stage_named_package(
        root,
        "first",
        "org.deckweave.fixture.first",
        "first-group",
        "first-device",
    )?;
    let (mut draft, original) = multi_runtime_fixture(hash.clone(), hash.clone())?;
    let mut graph: GraphDocument = serde_json::from_value(draft.document.clone())?;
    let mut endpoints = original.catalog().endpoints().to_vec();
    let mut schemas = original.catalog().schemas().to_vec();
    let settings_schema: deckweave_domain::schema::SchemaRef = serde_json::from_value(
        json!({"id":"org.fixture.account-settings","version":{"major":1,"minor":0}}),
    )?;
    schemas.push(CatalogSchema { schema: settings_schema.clone(), value_kind: SchemaValueKind::Structured(
        serde_json::from_value(json!({"type":"record","fields":{"account":{"schema":{"type":"string","maximumBytes":64}}}}))?), unit: None, range: None });
    let requirement = endpoints[0].runtime_requirement.clone();
    for (index, entry) in endpoints.iter_mut().enumerate() {
        let instance_id = PluginInstanceId::try_new(format!("account-{index}"))?;
        entry.runtime_requirement = requirement.clone();
        entry.endpoint.package_id = requirement.package_id.clone();
        if let EndpointTargetRef::Device {
            device_identity, ..
        } = &mut entry.endpoint.target
        {
            *device_identity = DeviceIdentity::try_new(instance_id.to_string())?;
        }
        let node = &mut graph.graph.nodes[index];
        if let GraphNodeKind::Source { endpoint, .. } = &mut node.kind {
            *endpoint = entry.endpoint.clone();
        }
        graph
            .graph
            .node_instances
            .insert(node.id.clone(), instance_id.clone());
        graph.plugin_instances.insert(
            instance_id.clone(),
            PluginInstanceConfiguration {
                format_version: 1,
                instance_id,
                package_id: requirement.package_id.clone(),
                contribution_id: requirement.contribution_id.clone(),
                settings_revision: index as u64 + 1,
                settings_schema: settings_schema.clone(),
                settings: json!({"account":format!("user-{index}")}),
                credentials: BTreeMap::new(),
                service_bindings: BTreeMap::new(),
                required_features: BTreeMap::new(),
                extensions: BTreeMap::new(),
            },
        );
    }
    draft.document = serde_json::to_value(graph)?;
    draft.package_lock.truncate(1);
    let package_root = root.join("packages/first/0.1.0/content");
    let manifest_path = package_root.join("deckweave-plugin.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    manifest["org.deckweave.automationCatalog"] =
        json!({"formatVersion":1,"schemas":schemas,"endpoints":endpoints});
    fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
    let packages = Arc::new(PackageIndex::scan(
        &root.join("packages"),
        &StagingLimits::default(),
    )?);
    let compiler =
        Arc::new(deckweave_plugin_supervisor::package::PackageGraphCompiler::new(packages));
    let repository = Arc::new(JsonProfileStore::new(
        root.join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    repository.create_draft(&draft)?;
    let authorization = Arc::new(MutableAuthorization::new());
    let (runtime, apply, registry) = build_runtime_with_compiler(
        root,
        repository.clone(),
        authorization.clone(),
        "configured-1",
        compiler.clone(),
    )?;
    let (gateway, context) = gateway(root, repository.clone(), apply, runtime.clone())?;
    let window = gateway.open_mutation_window(&context)?;
    let AppQueryResult::ApplyPlan(plan) = gateway.query(
        &context,
        AppQuery::PlanApply {
            draft_id: draft.draft_id.clone(),
            expected_draft_version: draft.version,
            expected_active: None,
            allow_disruption: false,
        },
    )?
    else {
        return Err("plan".into());
    };
    let AppCommandResult::OperationAccepted(receipt) = gateway.command(
        &context,
        CommandEnvelope {
            request_id: "configured".into(),
            idempotency_key: IdempotencyKey::try_new("configured")?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: draft.draft_id.clone(),
                expected_draft_version: draft.version,
                expected_active: None,
                allow_disruption: false,
                plan_hash: plan.plan_hash,
            },
        },
    )?
    else {
        return Err("apply".into());
    };
    assert_eq!(
        repository.operation(&receipt.operation_id)?.state,
        ApplyOperationState::Succeeded
    );
    assert_eq!(runtime.runtime_health().runtimes.len(), 1);
    assert_eq!(
        registry.lock().map_err(|_| "registry")?.snapshot()?.nodes[0]
            .descriptor
            .devices
            .len(),
        2
    );
    runtime.set_locale(LocaleContext {
        revision: 2,
        requested_locale: "ru-RU".into(),
        time_zone: "Asia/Yekaterinburg".into(),
    })?;
    let locale_path = package_root.join("bin/locale.json");
    wait_until(|| {
        fs::read(&locale_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .is_some_and(|v| v["revision"] == 2)
    })?;
    runtime.shutdown()?;
    drop(gateway);
    drop(runtime);
    let (runtime, apply, _) =
        build_runtime_with_compiler(root, repository, authorization, "configured-2", compiler)?;
    apply.recover(16)?;
    runtime.shutdown()?;
    let records = fs::read_to_string(package_root.join("bin/activations.jsonl"))?
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(records.len(), 4);
    for chunk in records.as_chunks::<2>().0 {
        assert_eq!(chunk[0]["pid"], chunk[1]["pid"]);
        assert_eq!(chunk[0]["settings"]["account"], "user-0");
        assert_eq!(chunk[1]["settings"]["account"], "user-1");
        assert_eq!(chunk[1]["revision"], 2);
    }
    Ok(())
}
