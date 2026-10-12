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

#[test]
fn configured_feedback_requires_explicit_dispatch_negotiation() -> Result<(), Box<dyn Error>> {
    let hub = RuntimeDispatchHub::new(Arc::new(
        deckweave_application::ScopedAuthorizationStore::default(),
    ));
    let epoch = ActivationEpoch::try_new(1)?;
    let target: RuntimeEndpointRef = serde_json::from_value(serde_json::json!({
        "stable":{"packageId":"org.fixture.configured","nodeId":"node","target":{"kind":"service","providerId":"provider","serviceId":"service"},"port":{"elementId":"button","capabilityId":"text","portId":"value"}},
        "runtimeGroupId":"control","pluginSessionId":"session","sessionGeneration":1,"descriptorRevision":1
    }))?;
    let key = WorkerKey::from_endpoint(&target);
    let (sender, receiver) = mpsc::sync_channel(1);
    {
        let mut state = hub.state.lock().map_err(|e| e.to_string())?;
        state.epoch = Some(epoch);
        state.closed = false;
        state.workers.insert(
            key.clone(),
            WorkerRoute {
                sender,
                grant_revision: hub.authorization.revision()?,
                features: BTreeMap::from([(FEATURE_VALUE_WRITE_ENDPOINT_PATH.into(), 1)]),
                failed_release: Arc::new(AtomicBool::new(false)),
            },
        );
    }
    let command = RuntimeCommand::Feedback {
        configured_instance_id: Some(deckweave_domain::ids::PluginInstanceId::try_new("work")?),
        run_id: deckweave_domain::ids::ExecutionId::try_new("run")?,
        graph_node_id: deckweave_domain::ids::GraphNodeId::try_new("feedback")?,
        activation_epoch: epoch,
        target,
        schema: serde_json::from_value(
            serde_json::json!({"id":"org.fixture.text","version":{"major":1,"minor":0}}),
        )?,
        value: serde_json::json!("test"),
    };
    assert_eq!(
        hub.dispatch(command.clone())
            .err()
            .ok_or("expected rejection")?
            .error
            .code,
        "configured_dispatch_not_negotiated"
    );
    assert!(receiver.try_recv().is_err());
    hub.state
        .lock()
        .map_err(|e| e.to_string())?
        .workers
        .get_mut(&key)
        .ok_or("worker")?
        .features
        .insert(deckweave_protocol::FEATURE_CONFIGURED_DISPATCH.into(), 1);
    hub.dispatch(command.clone()).map_err(|e| e.error.code)?;
    assert!(matches!(receiver.try_recv()?, WorkerCommand::Dispatch(actual) if *actual == command));
    Ok(())
}
