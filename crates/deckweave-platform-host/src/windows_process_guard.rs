// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]

use deckweave_application::{ChildProcessGuard, PlatformError};
use std::{mem::size_of, os::windows::io::AsRawHandle, process::Child, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    },
};

struct WindowsJobGuard {
    handle: HANDLE,
    terminated: bool,
}

unsafe impl Send for WindowsJobGuard {}

impl WindowsJobGuard {
    fn terminate(&mut self) -> Result<(), PlatformError> {
        if self.terminated {
            return Ok(());
        }
        // SAFETY: handle is an owned live job handle until Drop.
        if unsafe { TerminateJobObject(self.handle, 1) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        self.terminated = true;
        Ok(())
    }
}

impl ChildProcessGuard for WindowsJobGuard {
    fn terminate_tree(&mut self) -> Result<(), PlatformError> {
        self.terminate()
    }
}

impl Drop for WindowsJobGuard {
    fn drop(&mut self) {
        // KILL_ON_JOB_CLOSE is the final cleanup path even if explicit
        // termination was skipped because the direct child already exited.
        // SAFETY: handle is owned by this guard and closed exactly once.
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub fn attach(child: &mut Child) -> Result<Box<dyn ChildProcessGuard>, PlatformError> {
    // SAFETY: null name/security creates a private job with an owned handle.
    let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
    if job.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(|_| PlatformError::Unavailable("job information size exceeds u32".into()))?;
    // SAFETY: pointers reference initialized structures for the duration of the calls.
    let configured = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const information).cast(),
            size,
        )
    };
    if configured == 0 {
        let error = std::io::Error::last_os_error();
        // SAFETY: owned handle is closed on this error path.
        unsafe { CloseHandle(job) };
        return Err(error.into());
    }

    let process_handle = child.as_raw_handle() as HANDLE;
    // SAFETY: both handles are live; the process is assigned once immediately after spawn.
    if unsafe { AssignProcessToJobObject(job, process_handle) } == 0 {
        let error = std::io::Error::last_os_error();
        // SAFETY: owned handle is closed on this error path.
        unsafe { CloseHandle(job) };
        return Err(error.into());
    }
    Ok(Box::new(WindowsJobGuard {
        handle: job,
        terminated: false,
    }))
}
