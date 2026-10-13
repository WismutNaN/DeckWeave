// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn layout_revisions_retain_runs_but_graph_and_grant_changes_reactivate()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let (draft, compiler, marker) = m1_fixture::stage_and_build(root)?;
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
        "live-revisions",
        Arc::new(compiler),
    )?;
    let (gateway, context) = gateway(root, repository.clone(), apply, runtime.clone())?;
    let apply_revision = |version: u64| -> Result<String, Box<dyn Error>> {
        let expected_active = repository.active_commit()?.map(|commit| commit.candidate);
        let AppQueryResult::ApplyPlan(plan) = gateway.query(
            &context,
            AppQuery::PlanApply {
                draft_id: draft.draft_id.clone(),
                expected_draft_version: DraftVersion::try_new(version)?,
                expected_active: expected_active.clone(),
                allow_disruption: false,
            },
        )?
        else {
            return Err("plan".into());
        };
        let hash = plan.plan_hash.clone();
        let AppCommandResult::OperationAccepted(receipt) = gateway.command(
            &context,
            CommandEnvelope {
                request_id: format!("apply-{version}"),
                idempotency_key: IdempotencyKey::try_new(format!("apply-{version}"))?,
                dedup_epoch: gateway.open_mutation_window(&context)?.dedup_epoch,
                dry_run: false,
                command: AppCommand::ApplyDraft {
                    draft_id: draft.draft_id.clone(),
                    expected_draft_version: DraftVersion::try_new(version)?,
                    expected_active,
                    allow_disruption: false,
                    plan_hash: hash.clone(),
                },
            },
        )?
        else {
            return Err("apply".into());
        };
        assert_eq!(
            repository.operation(&receipt.operation_id)?.runtime_health,
            deckweave_workspace::ApplyRuntimeHealth::Applied
        );
        Ok(hash)
    };
    let patch = |version: u64, document: serde_json::Value| -> Result<(), Box<dyn Error>> {
        gateway.command(
            &context,
            CommandEnvelope {
                request_id: format!("patch-{version}"),
                idempotency_key: IdempotencyKey::try_new(format!("patch-{version}"))?,
                dedup_epoch: gateway.open_mutation_window(&context)?.dedup_epoch,
                dry_run: false,
                command: AppCommand::PatchDraft {
                    draft_id: draft.draft_id.clone(),
                    expected_version: DraftVersion::try_new(version)?,
                    edit: deckweave_workspace::DraftCommand::ReplaceDocument { document },
                },
            },
        )?;
        Ok(())
    };
    apply_revision(1)?;
    wait_until(|| fs::read_to_string(&marker).is_ok_and(|value| value.lines().count() == 2))?;
    let before = runtime.live_execution().snapshot()?.ok_or("scheduler")?;
    assert_eq!(before.completed.len(), 2);
    let topology = registry.lock().map_err(|_| "registry")?.snapshot()?;
    let mut document = draft.document.clone();
    document["layout"]["extensions"]["org.deckweave.editor"] = serde_json::json!({"x": 42});
    patch(1, document.clone())?;
    let hash = apply_revision(2)?;
    let after = runtime.live_execution().snapshot()?.ok_or("scheduler")?;
    assert_eq!(after.activation_epoch, before.activation_epoch);
    assert_eq!(after.completed, before.completed);
    assert_eq!(after.next_run_sequence, before.next_run_sequence);
    assert_eq!(after.values, before.values);
    assert_eq!(
        registry.lock().map_err(|_| "registry")?.snapshot()?,
        topology
    );
    let AppQueryResult::RuntimeTrace(trace) = gateway.query(&context, AppQuery::RuntimeTrace)?
    else {
        return Err("trace".into());
    };
    assert_eq!(trace.plan_hash.as_deref(), Some(hash.as_str()));
    assert!(
        !trace
            .entries
            .iter()
            .any(|entry| entry.kind == deckweave_application::RuntimeTraceEventKind::Barrier)
    );
    assert_eq!(fs::read_to_string(&marker)?.lines().count(), 2);

    // A changed executable input must use the ordinary activation path.
    let mut graph: GraphDocument = serde_json::from_value(document)?;
    let condition = graph
        .graph
        .nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "condition")
        .ok_or("condition")?;
    condition.kind = GraphNodeKind::Condition {
        operator: deckweave_automation::ConditionOperator::Equals,
        operand: Some(serde_json::json!(false)),
    };
    let document = serde_json::to_value(graph)?;
    patch(2, document.clone())?;
    apply_revision(3)?;
    let epoch = runtime
        .live_execution()
        .activation_epoch()?
        .ok_or("epoch")?;
    assert_ne!(epoch, before.activation_epoch);

    // Even identical behavior cannot reuse grants from an older revision.
    authorization.revision.fetch_add(1, Ordering::SeqCst);
    patch(3, document)?;
    apply_revision(4)?;
    assert_ne!(runtime.live_execution().activation_epoch()?, Some(epoch));
    runtime.shutdown()?;
    Ok(())
}
