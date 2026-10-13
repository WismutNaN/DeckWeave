// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_platform::{
    UserProcessFailure, UserProcessLauncher, UserProcessReceipt, UserProcessRequest,
};
use std::time::Instant;

#[derive(Debug, Default)]
pub struct NativeUserProcessLauncher;

impl UserProcessLauncher for NativeUserProcessLauncher {
    fn execute(
        &self,
        request: &UserProcessRequest,
        deadline: Instant,
    ) -> Result<UserProcessReceipt, UserProcessFailure> {
        request.validate()?;
        if Instant::now() >= deadline {
            return Err(UserProcessFailure::Deadline);
        }
        #[cfg(windows)]
        {
            native::execute(request, deadline)
        }
        #[cfg(not(windows))]
        {
            Err(UserProcessFailure::Unsupported)
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        os::windows::process::CommandExt,
        path::Path,
        process::{Command, Stdio},
        sync::{
            OnceLock,
            mpsc::{self, SyncSender},
        },
        thread,
    };
    struct Work {
        request: UserProcessRequest,
        deadline: Instant,
        reply: SyncSender<Result<UserProcessReceipt, UserProcessFailure>>,
    }
    static WORKER: OnceLock<Result<SyncSender<Work>, UserProcessFailure>> = OnceLock::new();
    pub(super) fn execute(
        request: &UserProcessRequest,
        deadline: Instant,
    ) -> Result<UserProcessReceipt, UserProcessFailure> {
        let sender = WORKER
            .get_or_init(|| {
                let (send, receive) = mpsc::sync_channel::<Work>(16);
                thread::Builder::new()
                    .name("deckweave-user-process".into())
                    .spawn(move || {
                        let apartment = crate::windows_shell::Apartment::new();
                        loop {
                            apartment.pump();
                            let work =
                                match receive.recv_timeout(std::time::Duration::from_millis(20)) {
                                    Ok(work) => work,
                                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                                    Err(_) => break,
                                };
                            let result = if Instant::now() >= work.deadline {
                                Err(UserProcessFailure::Deadline)
                            } else {
                                match work.request {
                                    UserProcessRequest::Launch(request) => {
                                        launch(&request, work.deadline)
                                    }
                                    UserProcessRequest::Open(request) => {
                                        apartment.open(&request, work.deadline)
                                    }
                                }
                            };
                            let _ = work.reply.try_send(result);
                        }
                    })
                    .map_err(|_| UserProcessFailure::Rejected)?;
                Ok(send)
            })
            .as_ref()
            .map_err(|e| *e)?;
        let (reply, receive) = mpsc::sync_channel(1);
        sender
            .try_send(Work {
                request: request.clone(),
                deadline,
                reply,
            })
            .map_err(|_| UserProcessFailure::Busy)?;
        receive
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| UserProcessFailure::UnknownOutcome)?
    }
    fn launch(
        request: &deckweave_platform::ExactLaunch,
        deadline: Instant,
    ) -> Result<UserProcessReceipt, UserProcessFailure> {
        let executable = Path::new(&request.executable);
        // Absolute .exe only: no PATH lookup, batch-file interpretation or shell concatenation.
        if !executable.is_absolute()
            || !executable
                .extension()
                .is_some_and(|v| v.eq_ignore_ascii_case("exe"))
            || request
                .working_directory
                .as_ref()
                .is_some_and(|v| !Path::new(v).is_absolute())
        {
            return Err(UserProcessFailure::InvalidInput);
        }
        let mut command = Command::new(executable);
        command
            .args(&request.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000);
        if let Some(cwd) = &request.working_directory {
            command.current_dir(cwd);
        }
        if Instant::now() >= deadline {
            return Err(UserProcessFailure::Deadline);
        }
        let child = command.spawn().map_err(|_| UserProcessFailure::Rejected)?;
        // Deliberately no ChildProcessPolicy::attach: this is the user's application.
        Ok(UserProcessReceipt {
            process_id: Some(child.id()),
        })
    }
}
