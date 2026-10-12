// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
pub(super) fn receive(
    instance: &mut RuntimeInstance,
    registry: &Arc<Mutex<CapabilityRegistry>>,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
    health: &RuntimeHealthStore,
    config: &HostRuntimeConfig,
    clock: Instant,
) -> bool {
    match instance.process.receive(config.monitor_interval) {
        Ok(envelope) => {
            if let Err(error) = instance
                .controller
                .accept_envelope(&envelope, elapsed_millis(clock))
            {
                if matches!(
                    error,
                    crate::supervisor::SupervisorError::LateSessionMessage
                ) {
                    return false;
                }
                instance
                    .failed_release
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                health.update(
                    instance
                        .health_entry(RuntimeHealthStatus::Unavailable, Some(error.to_string())),
                );
                topology_faults(instance, activation_epoch, sink);
                return true;
            }
            let session = instance
                .controller
                .session()
                .map(|value| value.identity.clone());
            let Some((session, body)) = session.zip(envelope.body.as_ref()) else {
                return false;
            };
            if let protocol_envelope::Body::OwnerCleanupReceipt(receipt) = body {
                if receipt.activation_epoch == activation_epoch.get()
                    && instance.cleanups.remove(&receipt.cleanup_id).is_some()
                {
                    sink.publish(RuntimeEvent {
                        activation_epoch,
                        kind: RuntimeEventKind::CleanupReceipt {
                            cleanup_id: receipt.cleanup_id.clone(),
                            released: receipt.released,
                        },
                    });
                    if !receipt.released {
                        instance
                            .failed_release
                            .store(true, std::sync::atomic::Ordering::SeqCst);
                        instance.cleanup_failed = true;
                        health.update(instance.health_entry(
                            RuntimeHealthStatus::Degraded,
                            Some("owner cleanup rejected".into()),
                        ));
                    }
                }
                return false;
            }
            match wire::event(
                &session,
                instance.descriptor_revision,
                activation_epoch,
                body,
                &instance.nodes,
                &mut instance.replies,
            ) {
                Ok(Some(event)) => sink.publish(event),
                Ok(None)
                    if matches!(
                        body,
                        protocol_envelope::Body::DescriptorSnapshot(_)
                            | protocol_envelope::Body::DescriptorDelta(_)
                    ) =>
                {
                    topology_faults(instance, activation_epoch, sink);
                    health.update(instance.health_entry(
                        RuntimeHealthStatus::Degraded,
                        Some("descriptor changed during active plan".into()),
                    ));
                }
                Ok(None) => {}
                Err(error) => health.update(
                    instance.health_entry(RuntimeHealthStatus::Degraded, Some(error.to_string())),
                ),
            }
        }
        Err(ProcessError::Timeout) => return false,
        Err(error) => {
            instance
                .failed_release
                .store(true, std::sync::atomic::Ordering::SeqCst);
            for (cleanup_id, _) in std::mem::take(&mut instance.cleanups) {
                sink.publish(RuntimeEvent {
                    activation_epoch,
                    kind: RuntimeEventKind::CleanupReceipt {
                        cleanup_id,
                        released: false,
                    },
                });
            }
            let now = elapsed_millis(clock);
            if let Ok(mut registry) = registry.lock() {
                let _ignored = instance
                    .controller
                    .process_exited(&mut registry, now, false);
            }
            health.update(instance.health_entry(
                RuntimeHealthStatus::Unavailable,
                Some(format!("plugin transport unavailable: {error}")),
            ));
            topology_faults(instance, activation_epoch, sink);
            return true;
        }
    }
    false
}

pub(super) fn process_stopped(
    instance: &mut RuntimeInstance,
    registry: &Arc<Mutex<CapabilityRegistry>>,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
    health: &RuntimeHealthStore,
    clock: Instant,
) -> bool {
    let now = elapsed_millis(clock);
    if instance.grants_changed().unwrap_or(true) {
        let action = registry
            .lock()
            .ok()
            .and_then(|mut registry| instance.revalidate_permissions(&mut registry, now).ok());
        if !matches!(action, Some(SupervisorAction::None)) {
            sink.publish(RuntimeEvent {
                activation_epoch,
                kind: RuntimeEventKind::GrantFault,
            });
            health.update(instance.health_entry(
                RuntimeHealthStatus::Unavailable,
                Some("runtime permission revoked".into()),
            ));
            return true;
        }
    }
    match instance.controller.tick(now) {
        Ok(SupervisorAction::ForceTerminate | SupervisorAction::RequestGracefulStop) => {
            topology_faults(instance, activation_epoch, sink);
            true
        }
        Ok(_) => false,
        Err(_) => true,
    }
}

pub(super) fn topology_faults(
    instance: &RuntimeInstance,
    activation_epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
) {
    for endpoint in instance.stable_endpoints() {
        sink.publish(RuntimeEvent {
            activation_epoch,
            kind: RuntimeEventKind::TopologyFault { endpoint },
        });
    }
}
