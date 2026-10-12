// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::virtual_control::*;
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Default)]
struct Clicks(AtomicUsize);
impl VirtualControlPort for Clicks {
    fn snapshot(&self) -> Result<VirtualSurfaceSnapshot, VirtualControlError> {
        Ok(VirtualSurfaceSnapshot::default())
    }
    fn click(&self, _: &VirtualClickRequest) -> Result<(), VirtualControlError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn virtual_click_requires_apply_and_never_reinjects_after_restart() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    let caller = context("editor", "virtual-click")?;
    authorizer.grant(
        caller.principal_id.clone(),
        [
            ApiAuthScope::Read,
            ApiAuthScope::Draft,
            ApiAuthScope::Simulation,
        ],
    );
    let clicks = Arc::new(Clicks::default());
    let gateway = service(directory.path().to_path_buf(), authorizer.clone())?
        .with_virtual_control(clicks.clone());
    let epoch = gateway.open_mutation_window(&caller)?.dedup_epoch;
    let request: VirtualClickRequest = serde_json::from_value(serde_json::json!({
        "endpoint": {"packageId":"org.deckweave.virtual","nodeId":"virtual-node","target":{"kind":"device","providerId":"virtual","deviceIdentity":"deck-a"},"port":{"elementId":"key-1","capabilityId":"press","portId":"pressed"}},
        "expectedActive":{"workspaceId":"workspace-1","revisionId":"revision-1"},"activationEpoch":1,"sessionGeneration":1
    }))?;
    let command = CommandEnvelope {
        request_id: "click".into(),
        idempotency_key: IdempotencyKey::try_new("click")?,
        dedup_epoch: epoch,
        dry_run: false,
        command: AppCommand::VirtualClick { request },
    };
    assert_eq!(
        gateway
            .command(&caller, command.clone())
            .err()
            .ok_or("simulation unexpectedly ran a live action")?
            .code,
        ApiErrorCode::PermissionDenied
    );
    assert_eq!(clicks.0.load(Ordering::SeqCst), 0);
    authorizer.grant(
        caller.principal_id.clone(),
        [ApiAuthScope::Read, ApiAuthScope::Apply],
    );
    let mut dry = command.clone();
    dry.dry_run = true;
    assert_eq!(
        gateway
            .command(&caller, dry)
            .err()
            .ok_or("dry run unexpectedly injected input")?
            .code,
        ApiErrorCode::InvalidCommand
    );
    assert_eq!(clicks.0.load(Ordering::SeqCst), 0);
    assert!(matches!(
        gateway.command(&caller, command.clone())?,
        AppCommandResult::VirtualClick {
            accepted: true,
            replayed: false
        }
    ));
    drop(gateway);
    let reopened =
        service(directory.path().to_path_buf(), authorizer)?.with_virtual_control(clicks.clone());
    assert!(matches!(
        reopened.command(&caller, command)?,
        AppCommandResult::VirtualClick {
            accepted: false,
            replayed: true
        }
    ));
    assert_eq!(clicks.0.load(Ordering::SeqCst), 1);
    Ok(())
}
