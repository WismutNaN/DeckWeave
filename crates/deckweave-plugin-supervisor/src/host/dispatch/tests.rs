// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::{
    error::Error,
    sync::atomic::{AtomicBool, Ordering},
};

#[test]
fn cleanup_overflow_marks_failed_owner_and_still_releases_other_workers()
-> Result<(), Box<dyn Error>> {
    let hub = RuntimeDispatchHub::new(Arc::new(
        deckweave_application::ScopedAuthorizationStore::default(),
    ));
    let epoch = ActivationEpoch::try_new(1)?;
    let mut receivers = Vec::new();
    let mut flags = Vec::new();
    for index in 0..2 {
        let endpoint: RuntimeEndpointRef = serde_json::from_value(serde_json::json!({
            "stable":{"packageId":"org.fixture.held","nodeId":"node","target":{"kind":"service","providerId":"provider","serviceId":"service"},
                "port":{"elementId":"button","capabilityId":"held","portId":"invoke"}},
            "runtimeGroupId":"control","pluginSessionId":format!("session-{index}"),"sessionGeneration":1,"descriptorRevision":1
        }))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        if index == 0 {
            sender.send(WorkerCommand::Start)?;
        }
        let flag = Arc::new(AtomicBool::new(false));
        hub.state.lock().map_err(|e| e.to_string())?.workers.insert(
            WorkerKey::from_endpoint(&endpoint),
            WorkerRoute {
                sender,
                grant_revision: GrantRevision::try_new(1)?,
                features: BTreeMap::from([(deckweave_protocol::FEATURE_OWNER_CLEANUP.into(), 1)]),
                failed_release: flag.clone(),
            },
        );
        receivers.push(receiver);
        flags.push(flag);
    }
    hub.state.lock().map_err(|e| e.to_string())?.epoch = Some(epoch);
    // Ordinary dispatch is closed, but cleanup remains admitted independently for every worker.
    assert!(
        hub.dispatch(RuntimeCommand::CleanupAll {
            activation_epoch: epoch,
            reason: "barrier".into()
        })
        .is_err()
    );
    assert!(flags[0].load(Ordering::SeqCst));
    assert!(!flags[1].load(Ordering::SeqCst));
    assert!(
        matches!(receivers[1].try_recv()?, WorkerCommand::Dispatch(command) if matches!(*command, RuntimeCommand::CleanupAll { .. }))
    );
    Ok(())
}
