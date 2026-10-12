// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(not(windows))]
use super::unsupported_backend::NativeAudioBackend;
#[cfg(windows)]
use super::windows_backend::NativeAudioBackend;
use super::{AudioCommand, AudioCommandError, AudioFailure, AudioState, service::AudioEngine};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const COMMAND_CAPACITY: usize = 32;
const POLL_INTERVAL: Duration = Duration::from_millis(50);

enum WorkerCommand {
    Apply {
        command: AudioCommand,
        reply: SyncSender<Result<AudioState, AudioCommandError>>,
    },
    Refresh,
    Shutdown,
}

#[derive(Clone)]
struct StatePublisher {
    latest: Arc<Mutex<Option<AudioState>>>,
    wake: SyncSender<()>,
}

impl StatePublisher {
    fn publish(&self, state: AudioState) {
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some(state);
            let _ = self.wake.try_send(());
        }
    }
}

pub struct AudioStateFeed {
    latest: Arc<Mutex<Option<AudioState>>>,
    wake: Receiver<()>,
}

impl AudioStateFeed {
    pub fn recv(&self) -> Result<AudioState, AudioFailure> {
        loop {
            self.wake.recv().map_err(|_| AudioFailure::WorkerStopped)?;
            if let Ok(mut latest) = self.latest.lock()
                && let Some(state) = latest.take()
            {
                return Ok(state);
            }
        }
    }
}

#[derive(Clone)]
pub struct AudioClient {
    commands: SyncSender<WorkerCommand>,
}

impl AudioClient {
    pub fn apply(
        &self,
        command: AudioCommand,
        timeout: Duration,
    ) -> Result<AudioState, AudioCommandError> {
        let (reply, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(WorkerCommand::Apply { command, reply })
            .map_err(|error| AudioCommandError {
                failure: match error {
                    TrySendError::Full(_) => AudioFailure::ResourceBusy,
                    TrySendError::Disconnected(_) => AudioFailure::WorkerStopped,
                },
                effect: super::EffectKnowledge::NotApplied,
            })?;
        result
            .recv_timeout(timeout)
            .map_err(|_| AudioCommandError {
                failure: AudioFailure::WorkerStopped,
                effect: super::EffectKnowledge::PossiblyApplied,
            })?
    }

    pub fn refresh(&self) -> Result<(), AudioFailure> {
        self.commands
            .try_send(WorkerCommand::Refresh)
            .map_err(|error| match error {
                TrySendError::Full(_) => AudioFailure::ResourceBusy,
                TrySendError::Disconnected(_) => AudioFailure::WorkerStopped,
            })
    }
}

pub struct AudioWorker {
    client: AudioClient,
    join: Option<JoinHandle<()>>,
}

impl AudioWorker {
    pub fn spawn() -> (Self, AudioStateFeed) {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (wake, wake_receiver) = mpsc::sync_channel(1);
        let latest = Arc::new(Mutex::new(None));
        let publisher = StatePublisher {
            latest: Arc::clone(&latest),
            wake,
        };
        let join = thread::Builder::new()
            .name("deckweave-core-audio".into())
            .spawn(move || worker_loop(receiver, publisher))
            .ok();
        let client = AudioClient { commands };
        (
            Self { client, join },
            AudioStateFeed {
                latest,
                wake: wake_receiver,
            },
        )
    }

    #[must_use]
    pub fn client(&self) -> AudioClient {
        self.client.clone()
    }
}

impl Drop for AudioWorker {
    fn drop(&mut self) {
        let _ = self.client.commands.send(WorkerCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn worker_loop(commands: Receiver<WorkerCommand>, publisher: StatePublisher) {
    let mut engine = AudioEngine::new(NativeAudioBackend::new());
    let mut published_revision = 0;
    publish_changed(&publisher, engine.refresh(), &mut published_revision);
    loop {
        match commands.recv_timeout(POLL_INTERVAL) {
            Ok(WorkerCommand::Apply { command, reply }) => {
                let outcome = engine.apply(command);
                if let Ok(state) = &outcome {
                    publish_changed(&publisher, state.clone(), &mut published_revision);
                } else {
                    publish_changed(&publisher, engine.refresh(), &mut published_revision);
                }
                let _ = reply.send(outcome);
            }
            Ok(WorkerCommand::Refresh) => {
                publish_changed(&publisher, engine.refresh(), &mut published_revision);
            }
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                if engine.maintenance_due() {
                    publish_changed(&publisher, engine.refresh(), &mut published_revision);
                }
            }
        }
    }
}

fn publish_changed(publisher: &StatePublisher, state: AudioState, published_revision: &mut u64) {
    if state.revision != *published_revision {
        *published_revision = state.revision;
        publisher.publish(state);
    }
}
