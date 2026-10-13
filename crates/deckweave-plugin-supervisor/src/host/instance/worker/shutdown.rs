// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

impl ActiveWorker {
    pub(crate) fn shutdown(mut self, timeout: Duration) -> Result<(), HostRuntimeError> {
        let (done, receiver) = mpsc::sync_channel(1);
        match self.sender.try_send(WorkerCommand::Shutdown(done)) {
            Ok(()) => match receiver.recv_timeout(timeout) {
                Ok(result) => result?,
                // A concurrent revoke may finish its drain before receiving this
                // Shutdown command. The joined monitor result is authoritative;
                // disconnect alone proves neither success nor failed release.
                Err(mpsc::RecvTimeoutError::Disconnected) => {}
                Err(error) => return Err(HostRuntimeError::Shutdown(error.to_string())),
            },
            Err(mpsc::TrySendError::Disconnected(_)) => {}
            Err(error) => return Err(HostRuntimeError::Shutdown(error.to_string())),
        }
        if let Some(join) = self.join.take() {
            join.join()
                .map_err(|_| HostRuntimeError::Shutdown("runtime monitor panicked".into()))??;
        }
        if self
            .failed_release
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(HostRuntimeError::Shutdown(
                "runtime release was not confirmed".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn release_queued(
    commands: &mpsc::Receiver<WorkerCommand>,
    mut release: impl FnMut(&RuntimeCommand),
) -> Vec<mpsc::SyncSender<Result<(), HostRuntimeError>>> {
    let mut waiters = Vec::new();
    while let Ok(command) = commands.try_recv() {
        match command {
            WorkerCommand::Dispatch(command) => {
                if matches!(
                    command.as_ref(),
                    RuntimeCommand::Cancel { .. }
                        | RuntimeCommand::CleanupAll { .. }
                        | RuntimeCommand::CleanupRun { .. }
                ) {
                    release(command.as_ref());
                }
            }
            WorkerCommand::Shutdown(done) => waiters.push(done),
            WorkerCommand::Start | WorkerCommand::Locale(_) => {}
        }
    }
    waiters
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drain_and_stop(
    mut instance: RuntimeInstance,
    registry: &Arc<Mutex<CapabilityRegistry>>,
    epoch: ActivationEpoch,
    sink: &dyn RuntimeEventSink,
    health: &RuntimeHealthStore,
    config: &HostRuntimeConfig,
    clock: Instant,
) -> Result<(), HostRuntimeError> {
    instance.broker_tickets.revoke_all();
    let deadline = Instant::now() + config.stop_timeout;
    while !instance.cleanups.is_empty() && Instant::now() < deadline {
        if receive(&mut instance, registry, epoch, sink, health, config, clock) {
            break;
        }
    }
    let unconfirmed = instance.cleanup_failed || !instance.cleanups.is_empty();
    for (cleanup_id, _) in std::mem::take(&mut instance.cleanups) {
        sink.publish(RuntimeEvent {
            activation_epoch: epoch,
            kind: RuntimeEventKind::CleanupReceipt {
                cleanup_id,
                released: false,
            },
        });
    }
    if unconfirmed {
        instance
            .failed_release
            .store(true, std::sync::atomic::Ordering::SeqCst);
        health.update(instance.health_entry(
            RuntimeHealthStatus::Degraded,
            Some("owner release was not confirmed before stop".into()),
        ));
    }
    let mut registry = registry
        .lock()
        .map_err(|_| HostRuntimeError::State("registry lock poisoned".into()))?;
    let stopped = instance.stop(&mut registry, config.stop_timeout, elapsed_millis(clock));
    if unconfirmed {
        return Err(HostRuntimeError::Shutdown(
            "owner release unconfirmed".into(),
        ));
    }
    stopped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_queued_during_revoke_uses_monitor_result() -> Result<(), Box<dyn std::error::Error>>
    {
        for failed in [false, true] {
            let (sender, receiver) = mpsc::sync_channel(1);
            let join = thread::spawn(move || {
                // Revoke has drained its snapshot of commands. A concurrent shutdown
                // arrives while the worker is completing the already-started stop.
                let queued = receiver
                    .recv()
                    .map_err(|e| HostRuntimeError::Shutdown(e.to_string()))?;
                drop(queued); // its receipt sender closes without a reply
                if failed {
                    Err(HostRuntimeError::Shutdown("stop failed".into()))
                } else {
                    Ok(())
                }
            });
            let worker = ActiveWorker {
                failed_release: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                sender,
                key: WorkerKey {
                    package_id: PackageId::try_new("org.deckweave.fixture")?,
                    runtime_group_id: RuntimeGroupId::try_new("control")?,
                    plugin_session_id: PluginSessionId::try_new("session")?,
                    generation: SessionGeneration::try_new(1)?,
                },
                grant_revision: GrantRevision::try_new(1)?,
                features: BTreeMap::new(),
                join: Some(join),
            };
            let result = worker.shutdown(Duration::from_secs(2));
            assert_eq!(result.is_err(), failed);
            if let Err(error) = result {
                assert!(error.to_string().contains("stop failed"));
            }
        }
        Ok(())
    }

    #[test]
    fn fault_drain_keeps_releases_after_locale_and_shutdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let (send, receive) = mpsc::sync_channel(8);
        let (done, result) = mpsc::sync_channel(1);
        send.send(WorkerCommand::Locale(Default::default()))?;
        send.send(WorkerCommand::Shutdown(done))?;
        send.send(WorkerCommand::Start)?;
        send.send(WorkerCommand::Dispatch(Box::new(
            RuntimeCommand::CleanupAll {
                activation_epoch: ActivationEpoch::try_new(1)?,
                reason: "revoked".into(),
            },
        )))?;
        let mut releases = 0;
        let waiters = release_queued(&receive, |_| releases += 1);
        assert_eq!(releases, 1);
        assert_eq!(waiters.len(), 1);
        for waiter in waiters {
            waiter.send(Ok(()))?;
        }
        result.recv_timeout(Duration::from_secs(1))??;
        Ok(())
    }
}
