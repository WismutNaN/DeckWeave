// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn source_action_feedback_contributions_share_one_process_with_static_metadata()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let (mut draft, original, _) = m1_fixture::stage_and_build(root)?;
    let package_id = PackageId::try_new(m1_fixture::SOURCE_PACKAGE)?;
    let group = RuntimeGroupId::try_new("source-group")?;
    let mut endpoints = original.catalog().endpoints().to_vec();
    for endpoint in &mut endpoints {
        endpoint.endpoint.package_id = package_id.clone();
        endpoint.runtime_requirement.package_id = package_id.clone();
        endpoint.runtime_requirement.runtime_group_id = group.clone();
    }
    let mut graph: GraphDocument = serde_json::from_value(draft.document)?;
    for node in &mut graph.graph.nodes {
        match &mut node.kind {
            GraphNodeKind::Source { endpoint, .. }
            | GraphNodeKind::Action { endpoint, .. }
            | GraphNodeKind::Feedback { endpoint, .. } => endpoint.package_id = package_id.clone(),
            _ => {}
        }
    }
    draft.document = serde_json::to_value(graph)?;
    draft
        .package_lock
        .retain(|entry| entry.package_id == package_id);
    let package_root = root.join("packages/m1-source/0.1.0/content");
    let manifest_path = package_root.join("deckweave-plugin.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let contributions = manifest["contributions"]
        .as_array_mut()
        .ok_or("contributions")?;
    for id in ["m1-action", "m1-feedback"] {
        contributions.push(serde_json::json!({"id":id,"type":"org.deckweave.service_provider","runtimeGroupId":"source-group","descriptor":{"kind":"virtual"}}));
    }
    manifest["org.deckweave.automationCatalog"] = serde_json::json!({"formatVersion":1,"schemas":original.catalog().schemas(),"endpoints":endpoints});
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
    let (runtime, apply, registry) = build_runtime_with_compiler(
        root,
        repository.clone(),
        Arc::new(MutableAuthorization::new()),
        "grouped",
        compiler,
    )?;
    let (gateway, context) = gateway(root, repository.clone(), apply, runtime.clone())?;
    held_outputs::apply_current(&gateway, &context, &repository, &draft, "grouped-apply")?;
    wait_until(|| {
        runtime
            .live_execution()
            .snapshot()
            .ok()
            .flatten()
            .is_some_and(|s| s.completed.len() == 2)
    })?;
    assert_eq!(runtime.runtime_health().runtimes.len(), 1);
    assert_eq!(
        registry
            .lock()
            .map_err(|e| e.to_string())?
            .snapshot()?
            .nodes
            .len(),
        3
    );
    wait_until(|| {
        fs::read_to_string(package_root.join("feedback.received.json"))
            .is_ok_and(|s| s.contains("completed:"))
    })?;
    runtime.shutdown()?;
    Ok(())
}
