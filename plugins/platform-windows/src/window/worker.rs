// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(not(windows))]
use super::unsupported_backend::{ForegroundHook, NativeWindowBackend};
#[cfg(windows)]
use super::windows_backend::{ForegroundHook, NativeWindowBackend};
use super::{
    ActiveWindowState, WindowAvailability, WindowCatalog, WindowCommandError, WindowFailure,
    WindowReference, WindowSelector, WindowService,
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const COMMAND_CAPACITY: usize = 32;
const POLL_INTERVAL: Duration = Duration::from_millis(25);

enum WorkerCommand {
    Activate {
        target: WindowReference,
        timeout: Duration,
        reply: SyncSender<Result<ActiveWindowState, WindowCommandError>>,
    },
    Focus {
        selector: WindowSelector,
        timeout: Duration,
        reply: SyncSender<Result<ActiveWindowState, WindowCommandError>>,
    },
    Catalog {
        reply: SyncSender<Result<WindowCatalog, WindowFailure>>,
    },
    Refresh,
    Shutdown,
}

#[derive(Clone)]
struct StatePublisher {
    latest: Arc<Mutex<Option<ActiveWindowState>>>,
    wake: SyncSender<()>,
}

impl StatePublisher {
    fn publish(&self, state: ActiveWindowState) {
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some(state);
            let _ = self.wake.try_send(());
        }
    }
}

pub(crate) struct WindowStateFeed {
    latest: Arc<Mutex<Option<ActiveWindowState>>>,
    wake: Receiver<()>,
}

impl WindowStateFeed {
    pub(crate) fn recv(&self) -> Result<ActiveWindowState, WindowFailure> {
        loop {
            self.wake.recv().map_err(|_| WindowFailure::WorkerStopped)?;
            if let Ok(mut latest) = self.latest.lock()
                && let Some(state) = latest.take()
            {
                return Ok(state);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct WindowClient {
    commands: SyncSender<WorkerCommand>,
}

impl WindowClient {
    pub(crate) fn activate(
        &self,
        target: WindowReference,
        timeout: Duration,
    ) -> Result<ActiveWindowState, WindowCommandError> {
        let (reply, result) = mpsc::sync_channel(1);
        self.admit(WorkerCommand::Activate {
            target,
            timeout,
            reply,
        })?;
        result
            .recv_timeout(timeout)
            .map_err(|_| WindowCommandError::after_attempt(WindowFailure::WorkerStopped))?
    }

    pub(crate) fn focus(
        &self,
        selector: WindowSelector,
        timeout: Duration,
    ) -> Result<ActiveWindowState, WindowCommandError> {
        let (reply, result) = mpsc::sync_channel(1);
        self.admit(WorkerCommand::Focus {
            selector,
            timeout,
            reply,
        })?;
        result
            .recv_timeout(timeout)
            .map_err(|_| WindowCommandError::after_attempt(WindowFailure::WorkerStopped))?
    }

    pub(crate) fn catalog(&self, timeout: Duration) -> Result<WindowCatalog, WindowCommandError> {
        let (reply, result) = mpsc::sync_channel(1);
        self.admit(WorkerCommand::Catalog { reply })?;
        result
            .recv_timeout(timeout)
            .map_err(|_| WindowCommandError::before_effect(WindowFailure::WorkerStopped))?
            .map_err(WindowCommandError::before_effect)
    }

    pub(crate) fn refresh(&self) -> Result<(), WindowFailure> {
        self.commands
            .try_send(WorkerCommand::Refresh)
            .map_err(admission_failure)
    }

    fn admit(&self, command: WorkerCommand) -> Result<(), WindowCommandError> {
        self.commands
            .try_send(command)
            .map_err(|error| WindowCommandError::before_effect(admission_failure(error)))
    }
}

pub(crate) struct WindowWorker {
    client: WindowClient,
    join: Option<JoinHandle<()>>,
}

impl WindowWorker {
    pub(crate) fn spawn(generation: u64) -> (Self, WindowStateFeed) {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (wake, wake_receiver) = mpsc::sync_channel(1);
        let latest = Arc::new(Mutex::new(None));
        let publisher = StatePublisher {
            latest: Arc::clone(&latest),
            wake,
        };
        let join = thread::Builder::new()
            .name("deckweave-window-worker".into())
            .spawn(move || worker_loop(generation, receiver, publisher))
            .ok();
        let client = WindowClient { commands };
        (
            Self { client, join },
            WindowStateFeed {
                latest,
                wake: wake_receiver,
            },
        )
    }

    pub(crate) fn client(&self) -> WindowClient {
        self.client.clone()
    }
}

impl Drop for WindowWorker {
    fn drop(&mut self) {
        let _ = self.client.commands.send(WorkerCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn worker_loop(generation: u64, commands: Receiver<WorkerCommand>, publisher: StatePublisher) {
    let (hook, changes) = ForegroundHook::start();
    let mut service = WindowService::new(NativeWindowBackend::new(), generation);
    let mut published_revision = 0;
    publish_result(
        &publisher,
        service.refresh(),
        generation,
        &mut published_revision,
    );
    loop {
        match commands.recv_timeout(POLL_INTERVAL) {
            Ok(WorkerCommand::Activate {
                target,
                timeout,
                reply,
            }) => {
                let outcome = service.activate_reference(&target, timeout);
                if let Ok(state) = &outcome {
                    publish_changed(&publisher, state.clone(), &mut published_revision);
                }
                let _ = reply.send(outcome);
            }
            Ok(WorkerCommand::Focus {
                selector,
                timeout,
                reply,
            }) => {
                let outcome = service.focus_selector(&selector, timeout);
                if let Ok(state) = &outcome {
                    publish_changed(&publisher, state.clone(), &mut published_revision);
                }
                let _ = reply.send(outcome);
            }
            Ok(WorkerCommand::Catalog { reply }) => {
                let _ = reply.send(service.catalog());
            }
            Ok(WorkerCommand::Refresh) => publish_result(
                &publisher,
                service.refresh(),
                generation,
                &mut published_revision,
            ),
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        while let Ok(()) = changes.try_recv() {
            publish_result(
                &publisher,
                service.refresh(),
                generation,
                &mut published_revision,
            );
        }
    }
    drop(hook);
}

fn publish_result(
    publisher: &StatePublisher,
    state: Result<ActiveWindowState, WindowFailure>,
    generation: u64,
    published_revision: &mut u64,
) {
    let state = state.unwrap_or_else(|failure| ActiveWindowState {
        availability: if failure == WindowFailure::UnsupportedOperatingSystem {
            WindowAvailability::Unsupported
        } else {
            WindowAvailability::NoForeground
        },
        generation: generation.max(1),
        window_ref: None,
        process_id: None,
        executable: None,
        title: String::new(),
        fresh: false,
        revision: published_revision.saturating_add(1),
    });
    publish_changed(publisher, state, published_revision);
}

fn publish_changed(
    publisher: &StatePublisher,
    state: ActiveWindowState,
    published_revision: &mut u64,
) {
    if state.revision != *published_revision {
        *published_revision = state.revision;
        publisher.publish(state);
    }
}

fn admission_failure<T>(error: TrySendError<T>) -> WindowFailure {
    match error {
        TrySendError::Full(_) => WindowFailure::ResourceBusy,
        TrySendError::Disconnected(_) => WindowFailure::WorkerStopped,
    }
}
