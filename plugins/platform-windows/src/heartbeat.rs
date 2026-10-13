// SPDX-License-Identifier: GPL-3.0-or-later
use crate::wire::WireSender;
use deckweave_protocol::v1::{Heartbeat, protocol_envelope};
use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) struct HeartbeatWorker {
    stop: mpsc::Sender<()>,
    join: Option<JoinHandle<()>>,
}

impl HeartbeatWorker {
    pub fn start(wire: WireSender) -> std::io::Result<Self> {
        let (stop, receiver) = mpsc::channel();
        let join = thread::Builder::new()
            .name("windows-heartbeat".into())
            .spawn(move || {
                let clock = Instant::now();
                while matches!(
                    receiver.recv_timeout(Duration::from_secs(1)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    if wire
                        .send(protocol_envelope::Body::Heartbeat(Heartbeat {
                            host_ticks: u64::try_from(clock.elapsed().as_millis())
                                .unwrap_or(u64::MAX),
                            active_jobs: 0,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            stop,
            join: Some(join),
        })
    }
}

impl Drop for HeartbeatWorker {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
