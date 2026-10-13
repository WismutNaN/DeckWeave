// SPDX-License-Identifier: GPL-3.0-or-later
use std::time::{Duration, Instant};
const INITIAL: Duration = Duration::from_millis(250);
const MAXIMUM: Duration = Duration::from_secs(5);
/// Only reconnection/observation is retried; this never owns an audio command.
pub(super) struct Recovery {
    next: Option<Instant>,
    delay: Duration,
}
impl Recovery {
    pub fn new() -> Self {
        Self {
            next: None,
            delay: INITIAL,
        }
    }
    pub fn due(&self, now: Instant) -> bool {
        self.next.is_none_or(|next| now >= next)
    }
    pub fn failed(&mut self, now: Instant) {
        self.next = Some(now + self.delay);
        self.delay = (self.delay * 2).min(MAXIMUM);
    }
    pub fn recovered(&mut self) {
        *self = Self::new();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_backend_recovers_with_bounded_backoff_and_resets_after_success() {
        let start = Instant::now();
        let mut now = start;
        let mut recovery = Recovery::new();
        for delay in [250, 500, 1000, 2000, 4000, 5000, 5000] {
            assert!(recovery.due(now));
            recovery.failed(now);
            now += Duration::from_millis(delay);
            assert!(!recovery.due(now - Duration::from_millis(1)));
            assert!(recovery.due(now));
        }
        recovery.recovered();
        recovery.failed(now);
        assert!(recovery.due(now + INITIAL));
    }
}
