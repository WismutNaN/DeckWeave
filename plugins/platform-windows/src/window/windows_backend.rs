// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]
use super::{NativeWindow, WindowBackend, WindowCommandError, WindowFailure};
use std::{
    ffi::c_void,
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, FILETIME, HWND, LPARAM, WPARAM},
        System::{
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{
                GetCurrentThreadId, GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            WindowsAndMessaging::{
                DispatchMessageW, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_NAMECHANGE, EVENT_SYSTEM_FOREGROUND, EnumWindows, GW_OWNER,
                GetForegroundWindow, GetMessageW, GetWindow, GetWindowTextW,
                GetWindowThreadProcessId, IsWindow, IsWindowVisible, MSG, OBJID_WINDOW,
                PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SW_RESTORE, SetForegroundWindow,
                ShowWindowAsync, TranslateMessage, WINEVENT_OUTOFCONTEXT, WM_QUIT,
            },
        },
    },
    core::{BOOL, PWSTR},
};

const TITLE_UNITS: usize = 513;
const PATH_UNITS: usize = 32_768;
const EXPORTED_PATH_BYTES: usize = 2_048;
const VERIFY_CAP: Duration = Duration::from_millis(250);

pub(super) struct NativeWindowBackend {
    session_id: Option<u32>,
    lifetimes: WindowLifetimes,
}

impl NativeWindowBackend {
    pub(super) fn new(hook: &ForegroundHook) -> Self {
        Self {
            session_id: process_session(std::process::id()),
            lifetimes: hook.lifetimes.clone(),
        }
    }
}

impl WindowBackend for NativeWindowBackend {
    fn list_windows(&mut self) -> Result<Vec<NativeWindow>, WindowFailure> {
        let mut context = Enumeration {
            session_id: self.session_id,
            windows: Vec::new(),
        };
        // SAFETY: EnumWindows is synchronous and context remains live until it returns.
        unsafe {
            EnumWindows(
                Some(collect_window),
                LPARAM((&raw mut context).cast::<c_void>() as isize),
            )
        }
        .map_err(map_error)?;
        self.lifetimes.assign(&mut context.windows)?;
        Ok(context.windows)
    }

    fn foreground_token(&mut self) -> Result<Option<u64>, WindowFailure> {
        // SAFETY: GetForegroundWindow returns a borrowed HWND or null.
        let hwnd = unsafe { GetForegroundWindow() };
        Ok((!hwnd.0.is_null()).then(|| token(hwnd)))
    }

    fn activate(
        &mut self,
        window: &NativeWindow,
        timeout: Duration,
    ) -> Result<(), WindowCommandError> {
        let stale = || WindowCommandError::before_effect(WindowFailure::StaleReference);
        let hwnd = handle(window.token);
        // Revalidate identity immediately before any OS effect, including process-start evidence.
        let mut current = describe_window(hwnd, self.session_id).ok_or_else(stale)?;
        current.incarnation = window.incarnation;
        if !super::service::same_identity(&current, window) || !self.lifetimes.matches(window) {
            return Err(stale());
        }
        // SAFETY: HWND still belongs to the revalidated window; Windows owns it.
        unsafe {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
        let deadline = Instant::now() + timeout.min(VERIFY_CAP);
        loop {
            if !self.lifetimes.matches(window) {
                return Err(WindowCommandError::after_attempt(
                    WindowFailure::StaleReference,
                ));
            }
            // SAFETY: GetForegroundWindow returns a borrowed HWND.
            if unsafe { GetForegroundWindow() } == hwnd {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(WindowCommandError::after_attempt(
                    WindowFailure::ForegroundDenied,
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

struct Enumeration {
    session_id: Option<u32>,
    windows: Vec<NativeWindow>,
}

unsafe extern "system" fn collect_window(hwnd: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: pointer was created from the live Enumeration passed to synchronous EnumWindows.
    let context = unsafe { &mut *(parameter.0 as *mut Enumeration) };
    if context.windows.len() >= 256 {
        return BOOL(1);
    }
    if let Some(window) = describe_window(hwnd, context.session_id) {
        context.windows.push(window);
    }
    BOOL(1)
}

fn describe_window(hwnd: HWND, session_id: Option<u32>) -> Option<NativeWindow> {
    // SAFETY: query functions borrow a HWND owned by Windows.
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool()
        || !unsafe { IsWindowVisible(hwnd) }.as_bool()
        || unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok()
    {
        return None;
    }
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut process_id)) };
    if process_id == 0
        || session_id.is_some_and(|session| process_session(process_id) != Some(session))
    {
        return None;
    }
    let (executable, process_started_at) = process_evidence(process_id);
    let mut title = [0_u16; TITLE_UNITS];
    let read = unsafe { GetWindowTextW(hwnd, &mut title) }.max(0) as usize;
    Some(NativeWindow {
        token: token(hwnd),
        incarnation: 0,
        process_id,
        process_started_at,
        executable,
        title: String::from_utf16_lossy(&title[..read]),
    })
}

fn process_session(process_id: u32) -> Option<u32> {
    let mut session = 0;
    // SAFETY: session points to initialized writable memory.
    unsafe { ProcessIdToSessionId(process_id, &raw mut session) }
        .ok()
        .map(|()| session)
}

fn process_evidence(process_id: u32) -> (Option<String>, Option<u64>) {
    // SAFETY: OpenProcess returns an owned handle on success.
    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return (None, None);
    };
    let mut path = [0_u16; PATH_UNITS];
    let mut length = u32::try_from(path.len()).unwrap_or(u32::MAX);
    // SAFETY: process is live and buffer/length are valid for the call.
    let executable = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &raw mut length,
        )
    }
    .ok()
    .map(|()| String::from_utf16_lossy(&path[..length as usize]))
    .filter(|value| value.len() <= EXPORTED_PATH_BYTES);
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: output pointers are valid and process is live.
    let started = unsafe {
        GetProcessTimes(
            process,
            &raw mut created,
            &raw mut exited,
            &raw mut kernel,
            &raw mut user,
        )
    }
    .ok()
    .map(|()| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime));
    // SAFETY: process is an owned handle closed exactly once.
    let _ = unsafe { CloseHandle(process) };
    (executable, started)
}

fn token(hwnd: HWND) -> u64 {
    hwnd.0 as usize as u64
}

fn handle(value: u64) -> HWND {
    HWND(value as usize as *mut c_void)
}

fn map_error(error: windows_core::Error) -> WindowFailure {
    match error.code().0 as u32 {
        0x8007_0005 => WindowFailure::PermissionDenied,
        _ => WindowFailure::OsApiUnavailable,
    }
}

mod hook;
mod lifetimes;
pub(super) use hook::ForegroundHook;
use lifetimes::WindowLifetimes;
