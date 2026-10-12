// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn negative_and_missing_cleanup_ack_never_report_successful_shutdown() -> Result<(), Box<dyn Error>>
{
    for mode in ["reject", "ignore"] {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let (draft, compiler, _) = m1_fixture::stage_and_build(root)?;
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
            "cleanup-test",
            Arc::new(compiler),
        )?;
        let (gateway, context) = gateway(root, repository, apply, runtime.clone())?;
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
        gateway.command(
            &context,
            CommandEnvelope {
                request_id: "cleanup".into(),
                idempotency_key: IdempotencyKey::try_new("cleanup")?,
                dedup_epoch: window.dedup_epoch,
                dry_run: false,
                command: AppCommand::ApplyDraft {
                    draft_id: draft.draft_id,
                    expected_draft_version: draft.version,
                    expected_active: None,
                    allow_disruption: false,
                    plan_hash: plan.plan_hash,
                },
            },
        )?;
        wait_until(|| {
            runtime
                .live_execution()
                .snapshot()
                .ok()
                .flatten()
                .is_some_and(|s| s.completed.len() == 2)
        })?;
        let index = PackageIndex::scan(&root.join("packages"), &StagingLimits::default())?;
        for package in index.packages() {
            fs::write(package.package_root.join("bin/cleanup-mode"), mode)?;
        }
        assert!(runtime.shutdown().is_err(), "{mode} must not be success");
        assert!(
            registry
                .lock()
                .map_err(|_| "registry")?
                .snapshot()?
                .nodes
                .is_empty()
        );
        let AppQueryResult::RuntimeTrace(trace) =
            gateway.query(&context, AppQuery::RuntimeTrace)?
        else {
            return Err("trace".into());
        };
        assert!(
            trace
                .entries
                .iter()
                .any(|e| e.outcome.as_deref() == Some("release_unconfirmed"))
        );
        assert!(
            runtime
                .runtime_health()
                .runtimes
                .iter()
                .any(|r| r.status == RuntimeHealthStatus::Degraded)
        );
    }
    Ok(())
}
