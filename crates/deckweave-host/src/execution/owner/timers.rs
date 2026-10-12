// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl LiveExecutionOwner {
    /// Ограничен maximum_pending_invocations; не требует новых сигналов provider.
    pub fn poll_deadlines(&self) -> Result<(), LiveExecutionError> {
        let now = self.clock.now_ticks();
        let expired = {
            let state = self
                .state
                .lock()
                .map_err(|_| LiveExecutionError::Poisoned)?;
            let Some(active) = state.active.as_ref() else {
                return Ok(());
            };
            active
                .scheduler
                .state()
                .pending
                .values()
                .filter(|pending| pending.deadline_ticks <= now)
                .map(|pending| (active.epoch, pending.invocation_id.clone()))
                .collect::<Vec<_>>()
        };
        for (epoch, invocation_id) in expired {
            self.ingest_kind(
                Some(epoch),
                IngressKind::TimerDeadline { invocation_id },
                now,
            )?;
        }
        Ok(())
    }
}
