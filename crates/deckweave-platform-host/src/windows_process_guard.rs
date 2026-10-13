// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]

use deckweave_platform::{ChildProcessGuard, PlatformError};
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
    let guard = WindowsJobGuard {
        handle: job,
        terminated: false,
    };
    // On any resume failure, dropping guard terminates the suspended process too.
    resume_primary_thread(child.id())?;
    Ok(Box::new(guard))
}

fn resume_primary_thread(process_id: u32) -> Result<(), PlatformError> {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };
    // SAFETY: snapshot is owned locally; no plugin thread has been resumed.
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: successful snapshot handle is uniquely owned here.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut thread_id = None;
    // SAFETY: initialized entry has its required structure size, snapshot is live.
    let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while found != 0 {
        if entry.th32OwnerProcessID == process_id && thread_id.replace(entry.th32ThreadID).is_some()
        {
            return Err(PlatformError::Unavailable(
                "suspended child has multiple threads".into(),
            ));
        }
        // SAFETY: entry/snapshot remain valid until enumeration completes.
        found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    let thread_id = thread_id
        .ok_or_else(|| PlatformError::Unavailable("suspended child thread missing".into()))?;
    // SAFETY: only the new child's primary thread is opened, with the minimal right.
    let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: successful thread handle is uniquely owned here.
    let thread = unsafe { OwnedHandle::from_raw_handle(handle) };
    // SAFETY: job assignment succeeded and this owned handle refers to its primary thread.
    match unsafe { ResumeThread(thread.as_raw_handle()) } {
        1 => Ok(()),
        u32::MAX => Err(std::io::Error::last_os_error().into()),
        _ => Err(PlatformError::Unavailable(
            "unexpected child suspend count".into(),
        )),
    }
}
