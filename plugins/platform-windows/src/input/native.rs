// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]
use super::normalizer::{Key, keys};
use std::{
    cell::RefCell,
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            Input::{
                GetRawInputData, HRAWINPUT,
                KeyboardAndMouse::{GetAsyncKeyState, MAPVK_VSC_TO_VK_EX, MapVirtualKeyW},
                RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT, RIDEV_DEVNOTIFY,
                RIDEV_INPUTSINK, RIDEV_REMOVE, RIM_TYPEKEYBOARD, RegisterRawInputDevices,
            },
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

pub(super) enum Record {
    Key {
        device: usize,
        scan: u16,
        flags: u16,
        vkey: u16,
        neutral: bool,
    },
    Removed(usize),
}

struct Context {
    sender: SyncSender<Record>,
    failed: Arc<AtomicBool>,
}
thread_local! { static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) }; }

pub(super) struct Source {
    id: u32,
    join: Option<JoinHandle<()>>,
}

pub(super) type Started = (
    Source,
    Receiver<Record>,
    Vec<Key>,
    BTreeSet<usize>,
    Arc<AtomicBool>,
);
impl Source {
    pub fn start() -> Result<Started, String> {
        let keys = keys().map_err(|e| e.to_string())?;
        let initial_keys = keys.clone();
        let (sender, receiver) = mpsc::sync_channel(256);
        let failed = Arc::new(AtomicBool::new(false));
        let worker_failed = failed.clone();
        let (ready, readiness) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("windows-raw-input".into())
            .spawn(move || {
                if message_loop(sender, worker_failed.clone(), &initial_keys, ready).is_err() {
                    worker_failed.store(true, Ordering::Release);
                }
            })
            .map_err(|e| e.to_string())?;
        // Initialization creates only a local message window and performs no external I/O.
        let result = readiness
            .recv_timeout(Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let (id, quarantined) = result?;
        Ok((
            Self {
                id,
                join: Some(join),
            },
            receiver,
            keys,
            quarantined,
            failed,
        ))
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: id belongs to the dedicated message thread with its queue already created.
        let _ = unsafe { PostThreadMessageW(self.id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

type Ready = SyncSender<Result<(u32, BTreeSet<usize>), String>>;
fn message_loop(
    sender: SyncSender<Record>,
    failed: Arc<AtomicBool>,
    keys: &[Key],
    ready: Ready,
) -> Result<(), String> {
    // SAFETY: all HWND operations and thread-local callback state remain on this owning thread.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?.into();
        let class = w!("DeckWeaveRawInput");
        let wc = WNDCLASSW {
            hInstance: instance,
            lpszClassName: class,
            lpfnWndProc: Some(window_proc),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("raw input window class".into());
        }
        let result = (|| {
            let hwnd = CreateWindowExW(
                Default::default(),
                class,
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance),
                None,
            )
            .map_err(|e| e.to_string())?;
            CONTEXT.with(|context| *context.borrow_mut() = Some(Context { sender, failed }));
            let registered = RegisterRawInputDevices(
                &[RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 6,
                    dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                    hwndTarget: hwnd,
                }],
                size_of::<RAWINPUTDEVICE>() as u32,
            );
            let result = match registered {
                Err(error) => {
                    let _ = ready.send(Err(error.to_string()));
                    Err(error.to_string())
                }
                Ok(()) => {
                    let quarantined = keys
                        .iter()
                        .enumerate()
                        .filter_map(|(i, k)| {
                            let code = u32::from(k.scan) | if k.extended { 0xe000 } else { 0 };
                            let vk = MapVirtualKeyW(code, MAPVK_VSC_TO_VK_EX);
                            (vk != 0 && GetAsyncKeyState(vk as i32) < 0).then_some(i)
                        })
                        .collect();
                    if ready.send(Ok((GetCurrentThreadId(), quarantined))).is_ok() {
                        let mut message = MSG::default();
                        loop {
                            let result = GetMessageW(&raw mut message, None, 0, 0);
                            if result.0 <= 0 {
                                break;
                            }
                            let _ = TranslateMessage(&message);
                            DispatchMessageW(&message);
                        }
                    }
                    let _ = RegisterRawInputDevices(
                        &[RAWINPUTDEVICE {
                            usUsagePage: 1,
                            usUsage: 6,
                            dwFlags: RIDEV_REMOVE,
                            hwndTarget: HWND::default(),
                        }],
                        size_of::<RAWINPUTDEVICE>() as u32,
                    );
                    Ok(())
                }
            };
            CONTEXT.with(|context| *context.borrow_mut() = None);
            let _ = DestroyWindow(hwnd);
            result
        })();
        let _ = UnregisterClassW(class, Some(instance));
        result
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_INPUT => {
            let mut raw = RAWINPUT::default();
            let mut bytes = size_of::<RAWINPUT>() as u32;
            // SAFETY: aligned initialized buffer and bounded size; inspect the union only for a keyboard record.
            let read = unsafe {
                GetRawInputData(
                    HRAWINPUT(lparam.0 as *mut _),
                    RID_INPUT,
                    Some((&raw mut raw).cast()),
                    &raw mut bytes,
                    size_of::<RAWINPUTHEADER>() as u32,
                )
            };
            if read != u32::MAX
                && read
                    >= (size_of::<RAWINPUTHEADER>()
                        + size_of::<windows::Win32::UI::Input::RAWKEYBOARD>())
                        as u32
                && raw.header.dwType == RIM_TYPEKEYBOARD.0
            {
                let key = unsafe { raw.data.keyboard };
                enqueue(Record::Key {
                    device: raw.header.hDevice.0 as usize,
                    scan: key.MakeCode,
                    flags: key.Flags,
                    vkey: key.VKey,
                    // SAFETY: read aggregate key state only to rearm initially held keys.
                    neutral: unsafe { GetAsyncKeyState(i32::from(key.VKey)) >= 0 },
                });
            }
        }
        WM_INPUT_DEVICE_CHANGE if wparam.0 == GIDC_REMOVAL as usize => {
            enqueue(Record::Removed(lparam.0 as usize))
        }
        _ => {}
    }
    // SAFETY: required for foreground WM_INPUT cleanup; legacy input is never suppressed.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn enqueue(record: Record) {
    CONTEXT.with(|context| {
        if let Some(context) = context.borrow().as_ref()
            && context.sender.try_send(record).is_err()
        {
            context.failed.store(true, Ordering::Release);
            // SAFETY: stop this message thread; the bridge reports a fatal source gap.
            unsafe {
                PostQuitMessage(1);
            }
        }
    });
}
