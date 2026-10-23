//! When to restart a managed Kanata, and when to give up (data-model › Backoff).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Restart delays in units (1 s by default), then the last one repeats.
const DELAYS: [u32; 6] = [1, 2, 4, 8, 16, 30];
/// Give up on this many failures within `WINDOW` (FR-007).
const MAX_FAILURES: usize = 5;
const WINDOW: Duration = Duration::from_secs(120);
/// This long running cleanly resets the delay.
const RESET_AFTER: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    Retry(Duration),
    GiveUp,
}

#[derive(Debug)]
pub(crate) struct Backoff {
    unit: Duration,
    failures: VecDeque<Instant>,
    consecutive: usize,
    running_since: Option<Instant>,
}

impl Backoff {
    pub(crate) fn new(unit: Duration) -> Self {
        Self {
            unit,
            failures: VecDeque::new(),
            consecutive: 0,
            running_since: None,
        }
    }

    /// Kanata is up and answering.
    pub(crate) fn running(&mut self, now: Instant) {
        self.running_since = Some(now);
    }

    /// Kanata failed at `now`: how long to wait, or give up.
    pub(crate) fn failure(&mut self, now: Instant) -> Next {
        if let Some(since) = self.running_since.take()
            && now.duration_since(since) >= RESET_AFTER
        {
            self.consecutive = 0;
        }
        self.failures.push_back(now);
        while self
            .failures
            .front()
            .is_some_and(|&t| now.duration_since(t) > WINDOW)
        {
            self.failures.pop_front();
        }
        if self.failures.len() >= MAX_FAILURES {
            return Next::GiveUp;
        }
        let units = DELAYS[self.consecutive.min(DELAYS.len() - 1)];
        self.consecutive += 1;
        Next::Retry(self.unit * units)
    }

    /// The user asked for a restart: forget the history (FR-009).
    pub(crate) fn clear(&mut self) {
        self.failures.clear();
        self.consecutive = 0;
        self.running_since = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn delays_grow_and_cap() {
        let mut b = Backoff::new(S);
        let t0 = Instant::now();
        let mut delays = Vec::new();
        // Spread failures out so the 2-minute give-up never triggers.
        for i in 0..8u64 {
            match b.failure(t0 + Duration::from_secs(i * 60)) {
                Next::Retry(d) => delays.push(d.as_secs()),
                Next::GiveUp => panic!("gave up at {i}"),
            }
        }
        assert_eq!(delays, [1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn five_in_two_minutes_gives_up() {
        let mut b = Backoff::new(S);
        let t0 = Instant::now();
        for i in 0..4u64 {
            assert!(matches!(b.failure(t0 + S * i as u32), Next::Retry(_)));
        }
        assert_eq!(b.failure(t0 + S * 4), Next::GiveUp);
    }

    #[test]
    fn spread_failures_dont_give_up() {
        let mut b = Backoff::new(S);
        let t0 = Instant::now();
        for i in 0..10u64 {
            assert!(
                matches!(b.failure(t0 + Duration::from_secs(i * 31)), Next::Retry(_)),
                "{i}"
            );
        }
    }

    #[test]
    fn a_minute_of_running_resets_the_delay() {
        let mut b = Backoff::new(S);
        let t0 = Instant::now();
        b.failure(t0);
        b.failure(t0 + S);
        b.running(t0 + S * 2);
        assert_eq!(b.failure(t0 + Duration::from_secs(70)), Next::Retry(S));
    }

    #[test]
    fn clear_forgets_everything() {
        let mut b = Backoff::new(S);
        let t0 = Instant::now();
        for i in 0..4 {
            b.failure(t0 + S * i);
        }
        b.clear();
        assert_eq!(b.failure(t0 + S * 5), Next::Retry(S));
    }
}
