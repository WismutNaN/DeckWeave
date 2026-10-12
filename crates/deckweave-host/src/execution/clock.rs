// SPDX-License-Identifier: GPL-3.0-or-later
//! Единая монотонная шкала host; ticks всегда означают миллисекунды.

use super::{LiveExecutionError, LiveExecutionOwner};
use std::{
    sync::{Arc, Weak, mpsc},
    thread,
    time::{Duration, Instant},
};

pub trait ExecutionClock: Send + Sync {
    fn now_ticks(&self) -> u64;
}

pub struct MonotonicExecutionClock(pub Instant);

impl ExecutionClock for MonotonicExecutionClock {
    fn now_ticks(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Владелец pump останавливает поток при shutdown/drop; поток не удерживает host.
pub struct ExecutionTimerPump {
    stop: mpsc::SyncSender<()>,
    join: Option<thread::JoinHandle<()>>,
}

impl ExecutionTimerPump {
    pub fn start(owner: &Arc<LiveExecutionOwner>, interval: Duration) -> Self {
        let owner: Weak<LiveExecutionOwner> = Arc::downgrade(owner);
        let (stop, receiver) = mpsc::sync_channel(1);
        let interval = interval.clamp(Duration::from_millis(1), Duration::from_secs(1));
        let join = thread::spawn(move || {
            while matches!(
                receiver.recv_timeout(interval),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                let Some(owner) = owner.upgrade() else {
                    break;
                };
                match owner.poll_deadlines() {
                    // Failed cleanup is already traced. Other owners still need deadlines;
                    // never retry a possibly delivered release as a new invocation.
                    Ok(()) | Err(LiveExecutionError::Cleanup(_)) => {}
                    Err(_) => break,
                }
            }
        });
        Self {
            stop,
            join: Some(join),
        }
    }
}

impl Drop for ExecutionTimerPump {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
