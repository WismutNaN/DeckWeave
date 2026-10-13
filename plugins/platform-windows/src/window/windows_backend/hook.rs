// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::cell::RefCell;
#[derive(Clone)]
struct HookContext {
    sender: SyncSender<()>,
    lifetimes: WindowLifetimes,
}
thread_local! { static HOOK_CONTEXT: RefCell<Option<HookContext>> = const { RefCell::new(None) }; }

pub(in crate::window) struct ForegroundHook {
    pub(super) lifetimes: WindowLifetimes,
    thread_id: Option<u32>,
    join: Option<JoinHandle<()>>,
}

impl ForegroundHook {
    pub(in crate::window) fn start() -> (Self, Receiver<()>) {
        let (events, receiver) = mpsc::sync_channel(1);
        let lifetimes = WindowLifetimes::default();
        let tracker = lifetimes.clone();
        let (ready, readiness) = mpsc::sync_channel(0);
        let join = thread::Builder::new()
            .name("deckweave-window-hook".into())
            .spawn(move || hook_loop(events, ready, tracker))
            .ok();
        let thread_id = readiness
            .recv_timeout(Duration::from_secs(1))
            .ok()
            .flatten();
        (
            Self {
                thread_id,
                join,
                lifetimes,
            },
            receiver,
        )
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

fn hook_loop(events: SyncSender<()>, ready: SyncSender<Option<u32>>, lifetimes: WindowLifetimes) {
    HOOK_CONTEXT.with(|context| {
        *context.borrow_mut() = Some(HookContext {
            sender: events,
            lifetimes: lifetimes.clone(),
        })
    });
    // SAFETY: the empty peek creates this thread's message queue before readiness is published.
    unsafe {
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let mut hooks = Vec::new();
    for (first, last) in [
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        (EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY),
        (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
    ] {
        // SAFETY: callback is static; every successful hook is unregistered on this thread.
        let hook = unsafe {
            SetWinEventHook(
                first,
                last,
                None,
                Some(foreground_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.0.is_null() {
            for previous in hooks {
                let _ = unsafe { UnhookWinEvent(previous) };
            }
            clear_hook_sender();
            let _ = ready.send(None);
            return;
        }
        hooks.push(hook);
    }
    lifetimes.set_active(true);
    // SAFETY: this is the current message thread identifier.
    let thread_id = unsafe { GetCurrentThreadId() };
    if ready.send(Some(thread_id)).is_err() {
        for hook in hooks {
            let _ = unsafe { UnhookWinEvent(hook) };
        }
        lifetimes.set_active(false);
        clear_hook_sender();
        return;
    }
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
        for hook in hooks {
            let _ = UnhookWinEvent(hook);
        }
    }
    lifetimes.set_active(false);
    clear_hook_sender();
}

unsafe extern "system" fn foreground_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if ![
        EVENT_SYSTEM_FOREGROUND,
        EVENT_OBJECT_CREATE,
        EVENT_OBJECT_DESTROY,
        EVENT_OBJECT_NAMECHANGE,
    ]
    .contains(&event)
        || object != OBJID_WINDOW.0
        || child != 0
    {
        return;
    }
    HOOK_CONTEXT.with(|context| {
        if let Some(context) = context.borrow().as_ref() {
            if event == EVENT_OBJECT_CREATE || event == EVENT_OBJECT_DESTROY {
                context.lifetimes.invalidate(token(hwnd));
            }
            let _ = context.sender.try_send(());
        }
    });
}

fn clear_hook_sender() {
    HOOK_CONTEXT.with(|context| *context.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_burst_coalesces_without_blocking() {
        let (sender, receiver) = mpsc::sync_channel(1);
        HOOK_CONTEXT.with(|context| {
            *context.borrow_mut() = Some(HookContext {
                sender,
                lifetimes: WindowLifetimes::default(),
            })
        });
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
