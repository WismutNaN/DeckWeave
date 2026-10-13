// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]
use deckweave_platform::{ShellTarget, UserProcessFailure, UserProcessReceipt};
use std::{path::Path, time::Instant};
use windows_sys::Win32::{
    Foundation::CloseHandle,
    System::{
        Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
        Threading::GetProcessId,
    },
    UI::{
        Shell::{
            SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
            ShellExecuteExW,
        },
        WindowsAndMessaging::{
            DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, SW_SHOWNORMAL, TranslateMessage,
        },
    },
};

pub(crate) struct Apartment {
    initialized: bool,
}
impl Apartment {
    pub fn new() -> Self {
        // SAFETY: constructed and retained only by the broker's owning STA thread.
        Self {
            initialized: unsafe {
                CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) >= 0
            },
        }
    }
    pub fn pump(&self) {
        // SAFETY: dispatch only the current thread's initialized message queue.
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&raw mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    pub fn open(
        &self,
        request: &ShellTarget,
        deadline: Instant,
    ) -> Result<UserProcessReceipt, UserProcessFailure> {
        if !self.initialized {
            return Err(UserProcessFailure::Rejected);
        }
        open(request, deadline)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.initialized {
            /* SAFETY: balances this thread's initialization. */
            unsafe {
                CoUninitialize();
            }
        }
    }
}

fn open(
    request: &ShellTarget,
    deadline: Instant,
) -> Result<UserProcessReceipt, UserProcessFailure> {
    let target = &request.target;
    let protocol = target.split_once(':').map(|v| v.0.to_ascii_lowercase());
    if !Path::new(target).is_absolute()
        && !matches!(protocol.as_deref(), Some("http" | "https" | "mailto"))
    {
        return Err(UserProcessFailure::InvalidInput);
    }
    let target: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // SAFETY: the broker owns a live STA apartment and these UTF-16 buffers throughout the call.
    unsafe {
        (|| {
            if Instant::now() >= deadline {
                return Err(UserProcessFailure::Deadline);
            }
            let mut info = SHELLEXECUTEINFOW {
                cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                lpVerb: verb.as_ptr(),
                lpFile: target.as_ptr(),
                nShow: SW_SHOWNORMAL,
                ..Default::default()
            };
            let success = ShellExecuteExW(&raw mut info) != 0;
            let process_id = if info.hProcess.is_null() {
                None
            } else {
                let id = GetProcessId(info.hProcess);
                CloseHandle(info.hProcess);
                (id != 0).then_some(id)
            };
            if success {
                Ok(UserProcessReceipt { process_id })
            } else {
                Err(UserProcessFailure::UnknownOutcome)
            }
        })()
    }
}
