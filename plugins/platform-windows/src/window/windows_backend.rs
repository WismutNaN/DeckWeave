// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]
use super::{NativeWindow, WindowBackend, WindowFailure};
use std::{
    ffi::c_void,
    sync::{
        Mutex, OnceLock,
        mpsc::{self, Receiver, SyncSender},
    },
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
                DispatchMessageW, EVENT_SYSTEM_FOREGROUND, EnumWindows, GW_OWNER,
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
}

impl NativeWindowBackend {
    pub(super) fn new() -> Self {
        Self {
            session_id: process_session(std::process::id()),
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
        Ok(context.windows)
    }

    fn foreground_token(&mut self) -> Result<Option<u64>, WindowFailure> {
        // SAFETY: GetForegroundWindow returns a borrowed HWND or null.
        let hwnd = unsafe { GetForegroundWindow() };
        Ok((!hwnd.0.is_null()).then(|| token(hwnd)))
    }

    fn activate(&mut self, value: u64, timeout: Duration) -> Result<(), WindowFailure> {
        let hwnd = handle(value);
        // SAFETY: HWND is validated before requesting activation; Windows owns it.
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err(WindowFailure::StaleReference);
        }
        // SAFETY: restoring and foregrounding a validated top-level HWND do not transfer ownership.
        unsafe {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
        let deadline = Instant::now() + timeout.min(VERIFY_CAP);
        loop {
            // SAFETY: GetForegroundWindow returns a borrowed HWND.
            if unsafe { GetForegroundWindow() } == hwnd {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(WindowFailure::ForegroundDenied);
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
    // SAFETY: query functions borrow a HWND owned by Windows.
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok() {
        return BOOL(1);
    }
    let mut process_id = 0;
    // SAFETY: process_id points to initialized writable memory.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut process_id)) };
    if process_id == 0
        || context
            .session_id
            .is_some_and(|session| process_session(process_id) != Some(session))
    {
        return BOOL(1);
    }
    let (executable, process_started_at) = process_evidence(process_id);
    let mut title = [0_u16; TITLE_UNITS];
    // SAFETY: buffer is writable and length is bounded by its slice.
    let read = unsafe { GetWindowTextW(hwnd, &mut title) }.max(0) as usize;
    context.windows.push(NativeWindow {
        token: token(hwnd),
        process_id,
        process_started_at,
        executable,
        title: String::from_utf16_lossy(&title[..read]),
    });
    BOOL(1)
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

static HOOK_SENDER: OnceLock<Mutex<Option<SyncSender<()>>>> = OnceLock::new();

pub(super) struct ForegroundHook {
    thread_id: Option<u32>,
    join: Option<JoinHandle<()>>,
}

impl ForegroundHook {
    pub(super) fn start() -> (Self, Receiver<()>) {
        let (events, receiver) = mpsc::sync_channel(1);
        let (ready, readiness) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("deckweave-window-hook".into())
            .spawn(move || hook_loop(events, ready))
            .ok();
        let thread_id = readiness
            .recv_timeout(Duration::from_secs(1))
            .ok()
            .flatten();
        (Self { thread_id, join }, receiver)
    }
}

impl Drop for ForegroundHook {
    fn drop(&mut self) {
        if let Some(thread_id) = self.thread_id {
            // SAFETY: posting WM_QUIT to the owned message thread transfers no memory.
            let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn hook_loop(events: SyncSender<()>, ready: SyncSender<Option<u32>>) {
    if let Ok(mut current) = HOOK_SENDER.get_or_init(|| Mutex::new(None)).lock() {
        *current = Some(events);
    }
    // SAFETY: the empty peek creates this thread's message queue before readiness is published.
    unsafe {
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    // SAFETY: callback is a static function and the hook is removed before thread exit.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(foreground_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    if hook.0.is_null() {
        clear_hook_sender();
        let _ = ready.send(None);
        return;
    }
    // SAFETY: this is the current message thread identifier.
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = ready.send(Some(thread_id));
    // SAFETY: message is initialized for each GetMessage iteration.
    unsafe {
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&raw mut message, None, 0, 0);
            if result.0 <= 0 {
                break;
            }
            let _ = TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
        let _ = UnhookWinEvent(hook);
    }
    clear_hook_sender();
}

unsafe extern "system" fn foreground_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    _hwnd: HWND,
    object: i32,
    child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if event != EVENT_SYSTEM_FOREGROUND || object != OBJID_WINDOW.0 || child != 0 {
        return;
    }
    if let Ok(current) = HOOK_SENDER.get_or_init(|| Mutex::new(None)).lock()
        && let Some(sender) = current.as_ref()
    {
        let _ = sender.try_send(());
    }
}

fn clear_hook_sender() {
    if let Ok(mut current) = HOOK_SENDER.get_or_init(|| Mutex::new(None)).lock() {
        *current = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_burst_coalesces_without_blocking() {
        let (sender, receiver) = mpsc::sync_channel(1);
        if let Ok(mut current) = HOOK_SENDER.get_or_init(|| Mutex::new(None)).lock() {
            *current = Some(sender);
        }
        for _ in 0..100 {
            // SAFETY: test supplies inert values; callback only touches the bounded sender.
            unsafe {
                foreground_event(
                    HWINEVENTHOOK::default(),
                    EVENT_SYSTEM_FOREGROUND,
                    HWND::default(),
                    OBJID_WINDOW.0,
                    0,
                    0,
                    0,
                );
            }
        }
        assert_eq!(receiver.try_iter().count(), 1);
        clear_hook_sender();
    }

    #[test]
    fn stable_failure_mapping_keeps_permission_distinct() {
        assert_eq!(
            map_error(windows_core::Error::from_hresult(windows_core::HRESULT(
                0x8007_0005_u32 as i32
            ))),
            WindowFailure::PermissionDenied
        );
    }
}
