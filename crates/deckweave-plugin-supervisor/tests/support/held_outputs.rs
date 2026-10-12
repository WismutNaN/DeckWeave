// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_domain::localization::LocaleContext;

fn apply_current(
    gateway: &ApplicationGateway,
    context: &RequestContext,
    repository: &JsonProfileStore,
    draft: &WorkspaceDraft,
    key: &str,
) -> Result<(), Box<dyn Error>> {
    let expected_active = repository.active_commit()?.map(|c| c.candidate);
    let AppQueryResult::ApplyPlan(plan) = gateway.query(
        context,
        AppQuery::PlanApply {
            draft_id: draft.draft_id.clone(),
            expected_draft_version: draft.version,
            expected_active: expected_active.clone(),
            allow_disruption: false,
        },
    )?
    else {
        return Err("plan".into());
    };
    let window = gateway.open_mutation_window(context)?;
    let AppCommandResult::OperationAccepted(receipt) = gateway.command(
        context,
        CommandEnvelope {
            request_id: key.into(),
            idempotency_key: IdempotencyKey::try_new(key)?,
            dedup_epoch: window.dedup_epoch,
            dry_run: false,
            command: AppCommand::ApplyDraft {
                draft_id: draft.draft_id.clone(),
                expected_draft_version: draft.version,
                expected_active,
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
    Ok(())
}
fn report(path: &Path) -> Option<Vec<serde_json::Value>> {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}
fn reports(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with("held-") && n.ends_with(".json"))
        })
        .collect()
}

#[test]
fn live_held_owners_release_independently_and_apply_revoke_crash_have_explicit_outcomes()
-> Result<(), Box<dyn Error>> {
    for ending in ["shutdown", "apply", "revoke", "crash", "overflow"] {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let (draft, compiler, _) = m1_fixture::stage_and_build(root)?;
        let mut endpoints = compiler.catalog().endpoints().to_vec();
        for endpoint in &mut endpoints {
            if let Some(operation) = endpoint.operation.as_mut() {
                operation.limits.deadline_ms = 30_000;
            }
        }
        let compiler = GraphPlanCompiler::new(
            DescriptorCatalog::try_new(compiler.catalog().schemas().to_vec(), endpoints)?,
            GraphLimits::default(),
        )?;
        let bin = root.join("packages/m1-action/0.1.0/content/bin");
        fs::write(bin.join("held-mode"), "hold")?;
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
            "held-test",
            Arc::new(compiler),
        )?;
        let (gateway, context) = gateway(root, repository.clone(), apply, runtime.clone())?;
        apply_current(&gateway, &context, &repository, &draft, "held-first")?;
        wait_until(|| {
            reports(&bin)
                .iter()
                .any(|p| report(p).is_some_and(|owners| owners.len() == 2))
        })?;
        let old_report = reports(&bin)
            .into_iter()
            .find(|p| report(p).is_some_and(|r| r.len() == 2))
            .ok_or("held report")?;
        let original = report(&old_report).ok_or("owners")?;
        fs::write(bin.join("held-complete-one"), "complete")?;
        runtime.set_locale(LocaleContext {
            revision: 2,
            requested_locale: "ru".into(),
            time_zone: "UTC".into(),
        })?;
        wait_until(|| report(&old_report).is_some_and(|owners| owners.len() == 1))?;
        let remaining = report(&old_report).ok_or("remaining")?;
        assert!(original.contains(&remaining[0]));
        assert_eq!(
            runtime
                .live_execution()
                .snapshot()?
                .ok_or("scheduler")?
                .pending
                .len(),
            1
        );
        match ending {
            "overflow" => {
                fs::write(
                    root.join("packages/m1-source/0.1.0/content/bin/mixed-flood"),
                    "flood",
                )?;
                fs::write(bin.join("held-complete-one"), "complete")?;
                runtime.set_locale(LocaleContext {
                    revision: 3,
                    requested_locale: "en".into(),
                    time_zone: "UTC".into(),
                })?;
                wait_until(|| {
                    runtime.runtime_health().runtimes.iter().any(|r| {
                        r.package_id.as_str() == m1_fixture::SOURCE_PACKAGE
                            && r.status == RuntimeHealthStatus::Unavailable
                    })
                })?;
                wait_until(|| {
                    runtime
                        .live_execution()
                        .snapshot()
                        .ok()
                        .flatten()
                        .is_some_and(|s| s.pending.is_empty())
                })?;
                wait_until(|| report(&old_report) == Some(vec![]))?;
                assert!(runtime.shutdown().is_err());
                assert!(runtime.runtime_health().runtimes.iter().any(|r| {
                    r.package_id.as_str() == m1_fixture::SOURCE_PACKAGE
                        && r.detail
                            .as_deref()
                            .is_some_and(|d| d.contains("bounded control queue"))
                }));
            }
            "apply" => {
                apply_current(&gateway, &context, &repository, &draft, "held-second")?;
                assert_eq!(report(&old_report), Some(vec![]));
                wait_until(|| {
                    reports(&bin)
                        .iter()
                        .any(|p| report(p).is_some_and(|owners| owners.len() == 2))
                })?;
                runtime.shutdown()?;
                assert!(reports(&bin).iter().all(|p| report(p) == Some(vec![])));
            }
            "revoke" => {
                authorization.revoke();
                wait_until(|| {
                    registry
                        .lock()
                        .ok()
                        .and_then(|r| r.snapshot().ok())
                        .is_some_and(|s| s.nodes.is_empty())
                })?;
                wait_until(|| report(&old_report) == Some(vec![]))?;
                let _ = runtime.shutdown(); // Another already-stopped route can report unconfirmed cleanup.
            }
            "crash" => {
                fs::write(bin.join("held-crash"), "crash")?;
                runtime.set_locale(LocaleContext {
                    revision: 3,
                    requested_locale: "en".into(),
                    time_zone: "UTC".into(),
                })?;
                wait_until(|| {
                    runtime.runtime_health().runtimes.iter().any(|r| {
                        r.package_id.as_str() == m1_fixture::ACTION_PACKAGE
                            && r.status == RuntimeHealthStatus::Unavailable
                    })
                })?;
                assert!(runtime.shutdown().is_err());
                // External held-state evidence remains: a dead provider cannot acknowledge release.
                assert_eq!(report(&old_report), Some(remaining));
            }
            _ => {
                runtime.shutdown()?;
                assert_eq!(report(&old_report), Some(vec![]));
            }
        }
    }
    Ok(())
}
