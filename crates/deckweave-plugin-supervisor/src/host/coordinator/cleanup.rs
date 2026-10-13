// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

impl ProductionRuntimeCoordinator {
    pub(super) fn stop_instances(
        &self,
        instances: Vec<RuntimeInstance>,
    ) -> Result<(), HostRuntimeError> {
        let mut first_error = None;
        for instance in instances {
            let result = self
                .registry
                .lock()
                .map_err(|_| HostRuntimeError::State("registry lock is poisoned".into()))
                .and_then(|mut registry| {
                    instance.stop(
                        &mut registry,
                        self.config.stop_timeout,
                        elapsed_millis(self.clock),
                    )
                });
            if first_error.is_none() {
                first_error = result.err();
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(super) fn shutdown_workers(
        &self,
        workers: Vec<ActiveWorker>,
    ) -> Result<(), HostRuntimeError> {
        let mut first_error = None;
        for worker in workers {
            let result = worker.shutdown(self.config.shutdown_timeout);
            if first_error.is_none() {
                first_error = result.err();
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}
