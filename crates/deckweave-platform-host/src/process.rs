// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_platform::{ChildProcessGuard, ChildProcessPolicy, PlatformError};
use std::process::{Child, Command};

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeChildProcessPolicy;

#[cfg(not(windows))]
struct NativeChildProcessGuard {
    process_group_id: i32,
    terminated: bool,
}

#[cfg(not(windows))]
impl NativeChildProcessGuard {
    fn kill_group(&mut self) -> Result<(), PlatformError> {
        use nix::{
            errno::Errno,
            sys::signal::{Signal, killpg},
            unistd::Pid,
        };

        if self.terminated {
            return Ok(());
        }

        match killpg(Pid::from_raw(self.process_group_id), Signal::SIGKILL) {
            Ok(()) | Err(Errno::ESRCH) => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        self.terminated = true;
        Ok(())
    }
}

#[cfg(not(windows))]
impl ChildProcessGuard for NativeChildProcessGuard {
    fn terminate_tree(&mut self) -> Result<(), PlatformError> {
        self.kill_group()
    }
}

#[cfg(not(windows))]
impl Drop for NativeChildProcessGuard {
    fn drop(&mut self) {
        let _ignored = self.kill_group();
    }
}

impl ChildProcessPolicy for NativeChildProcessPolicy {
    fn configure(&self, command: &mut Command) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            let _ = command;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            Ok(())
        }
    }

    fn attach(&self, child: &mut Child) -> Result<Box<dyn ChildProcessGuard>, PlatformError> {
        #[cfg(windows)]
        {
            crate::windows_process_guard::attach(child)
        }
        #[cfg(not(windows))]
        {
            let process_group_id = i32::try_from(child.id())
                .map_err(|_| PlatformError::Unavailable("child PID exceeds i32".into()))?;
            Ok(Box::new(NativeChildProcessGuard {
                process_group_id,
                terminated: false,
            }))
        }
    }
}
