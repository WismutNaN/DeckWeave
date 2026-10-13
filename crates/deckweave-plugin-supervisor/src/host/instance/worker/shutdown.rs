// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

impl ActiveWorker {
    pub(crate) fn shutdown(mut self, timeout: Duration) -> Result<(), HostRuntimeError> {
        let (done, receiver) = mpsc::sync_channel(1);
        match self.sender.try_send(WorkerCommand::Shutdown(done)) {
            Ok(()) => receiver
                .recv_timeout(timeout)
                .map_err(|error| HostRuntimeError::Shutdown(error.to_string()))??,
            Err(mpsc::TrySendError::Disconnected(_)) => {}
            Err(error) => return Err(HostRuntimeError::Shutdown(error.to_string())),
        }
        if let Some(join) = self.join.take() {
            join.join()
                .map_err(|_| HostRuntimeError::Shutdown("runtime monitor panicked".into()))?;
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
