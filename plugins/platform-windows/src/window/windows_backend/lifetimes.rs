// SPDX-License-Identifier: GPL-3.0-or-later
use super::{NativeWindow, WindowFailure};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Default)]
struct Book {
    next: u64,
    entries: BTreeMap<u64, u64>,
}
#[derive(Clone, Default)]
pub(super) struct WindowLifetimes {
    book: Arc<Mutex<Book>>,
    lost: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
}
impl WindowLifetimes {
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Release);
    }
    pub fn assign(&self, windows: &mut [NativeWindow]) -> Result<(), WindowFailure> {
        if !self.active.load(Ordering::Acquire) {
            return Err(WindowFailure::OsApiUnavailable);
        }
        if windows.len() > 256 {
            return Err(WindowFailure::ResourceBusy);
        }
        let mut book = self
            .book
            .lock()
            .map_err(|_| WindowFailure::OsApiUnavailable)?;
        if self.lost.swap(false, Ordering::AcqRel) {
            book.entries.clear();
        }
        book.entries
            .retain(|token, _| windows.iter().any(|w| w.token == *token));
        for window in windows {
            window.incarnation = if let Some(value) = book.entries.get(&window.token) {
                *value
            } else {
                book.next = book
                    .next
                    .checked_add(1)
                    .ok_or(WindowFailure::ResourceBusy)?;
                let value = book.next;
                book.entries.insert(window.token, value);
                value
            };
        }
        Ok(())
    }
    /// Called only for create/destroy; never waits for metadata enumeration.
    pub fn invalidate(&self, token: u64) {
        match self.book.try_lock() {
            Ok(mut book) => {
                book.entries.remove(&token);
            }
            Err(_) => self.lost.store(true, Ordering::Release),
        }
    }
    pub fn matches(&self, window: &NativeWindow) -> bool {
        self.active.load(Ordering::Acquire)
            && !self.lost.load(Ordering::Acquire)
            && self
                .book
                .lock()
                .is_ok_and(|book| book.entries.get(&window.token) == Some(&window.incarnation))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn window() -> NativeWindow {
        NativeWindow {
            token: 1,
            incarnation: 0,
            process_id: 2,
            process_started_at: Some(3),
            executable: None,
            title: "old".into(),
        }
    }
    #[test]
    fn title_changes_keep_identity_but_destroy_reuse_and_disappearance_do_not()
    -> Result<(), WindowFailure> {
        let tracker = WindowLifetimes::default();
        tracker.set_active(true);
        let mut windows = [window()];
        tracker.assign(&mut windows)?;
        let old = windows[0].clone();
        windows[0].title = "new".into();
        tracker.assign(&mut windows)?;
        assert_eq!(old.incarnation, windows[0].incarnation);
        tracker.invalidate(old.token);
        assert!(!tracker.matches(&old));
        tracker.assign(&mut windows)?;
        assert_ne!(old.incarnation, windows[0].incarnation);
        let reused = windows[0].clone();
        tracker.assign(&mut [])?;
        tracker.assign(&mut windows)?;
        assert_ne!(reused.incarnation, windows[0].incarnation);
        Ok(())
    }
    #[test]
    fn callback_contention_and_hook_loss_invalidate_references() -> Result<(), WindowFailure> {
        let tracker = WindowLifetimes::default();
        tracker.set_active(true);
        let mut windows = [window()];
        tracker.assign(&mut windows)?;
        let old = windows[0].clone();
        {
            let _guard = tracker
                .book
                .lock()
                .map_err(|_| WindowFailure::OsApiUnavailable)?;
            tracker.invalidate(old.token);
        }
        assert!(!tracker.matches(&old));
        tracker.assign(&mut windows)?;
        assert_ne!(old.incarnation, windows[0].incarnation);
        tracker.set_active(false);
        assert!(!tracker.matches(&windows[0]));
        assert!(tracker.assign(&mut windows).is_err());
        Ok(())
    }
}
