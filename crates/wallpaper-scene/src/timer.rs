//! Host-owned deadlines for script callbacks. Dispatch and lifecycle events are caller-owned.
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const MAX_TIMERS: usize = 64;
pub const MIN_DELAY: Duration = Duration::from_millis(16);
pub const MAX_DELAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimerId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Delay,
    BackwardsTime,
    Exhausted,
}

struct Timer {
    at: Instant,
    interval: Option<Duration>,
}

pub struct Timers {
    timers: BTreeMap<TimerId, Timer>,
    next: u64,
    observed: Instant,
    dispatch_after: Instant,
    paused: bool,
}

impl Timers {
    pub fn new(now: Instant) -> Self {
        Self {
            timers: BTreeMap::new(),
            next: 1,
            observed: now,
            dispatch_after: now,
            paused: false,
        }
    }

    pub fn schedule(
        &mut self,
        now: Instant,
        delay: Duration,
        repeating: bool,
    ) -> Result<TimerId, Error> {
        self.check_now(now)?;
        if !(MIN_DELAY..=MAX_DELAY).contains(&delay) {
            return Err(Error::Delay);
        }
        if self.timers.len() >= MAX_TIMERS {
            return Err(Error::Capacity);
        }
        let at = now.checked_add(delay).ok_or(Error::Exhausted)?;
        let id = TimerId(self.next);
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        self.timers.insert(
            id,
            Timer {
                at,
                interval: repeating.then_some(delay),
            },
        );
        self.observed = now;
        Ok(id)
    }

    pub fn cancel(&mut self, id: TimerId) -> bool {
        self.timers.remove(&id).is_some()
    }

    pub fn set_paused(&mut self, now: Instant, paused: bool) -> Result<(), Error> {
        self.check_now(now)?;
        self.paused = paused;
        self.observed = now;
        Ok(())
    }

    pub fn next_wake(&self) -> Option<Instant> {
        if self.paused {
            return None;
        }
        self.timers
            .values()
            .map(|timer| timer.at)
            .min()
            .map(|at| at.max(self.dispatch_after).max(self.observed))
    }

    /// Dispatch one timer at most. Repeating timers advance from the actual dispatch
    /// time, so missed intervals never become queued work.
    pub fn take_due(&mut self, now: Instant) -> Result<Option<TimerId>, Error> {
        self.check_now(now)?;
        if self.paused || now < self.dispatch_after {
            self.observed = now;
            return Ok(None);
        }
        let Some((&id, timer)) = self
            .timers
            .iter()
            .filter(|(_, timer)| timer.at <= now)
            .min_by_key(|(id, timer)| (timer.at, **id))
        else {
            self.observed = now;
            return Ok(None);
        };
        let dispatch_after = now.checked_add(MIN_DELAY).ok_or(Error::Exhausted)?;
        if let Some(interval) = timer.interval {
            let at = now.checked_add(interval).ok_or(Error::Exhausted)?;
            self.timers.insert(
                id,
                Timer {
                    at,
                    interval: Some(interval),
                },
            );
        } else {
            self.timers.remove(&id);
        }
        self.observed = now;
        self.dispatch_after = dispatch_after;
        Ok(Some(id))
    }

    fn check_now(&self, now: Instant) -> Result<(), Error> {
        if now < self.observed {
            return Err(Error::BackwardsTime);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_interval_runs_once_and_reschedules_from_resume_dispatch() {
        let start = Instant::now();
        let mut timers = Timers::new(start);
        let interval = Duration::from_secs(1);
        let id = timers.schedule(start, interval, true).unwrap();
        assert_eq!(timers.take_due(start).unwrap(), None);
        timers.set_paused(start + MIN_DELAY, true).unwrap();
        let resume = start + Duration::from_secs(60);
        assert_eq!(timers.next_wake(), None);
        assert_eq!(timers.take_due(resume).unwrap(), None);
        timers.set_paused(resume, false).unwrap();
        assert_eq!(timers.next_wake(), Some(resume));
        assert_eq!(timers.take_due(resume).unwrap(), Some(id));
        assert_eq!(timers.take_due(resume + MIN_DELAY).unwrap(), None);
        assert_eq!(timers.next_wake(), Some(resume + interval));
        assert_eq!(timers.take_due(resume + interval).unwrap(), Some(id));
    }

    #[test]
    fn same_deadline_one_shots_dispatch_in_order_at_bounded_cadence() {
        let start = Instant::now();
        let mut timers = Timers::new(start);
        let first = timers.schedule(start, MIN_DELAY, false).unwrap();
        let second = timers.schedule(start, MIN_DELAY, false).unwrap();
        let deadline = start + MIN_DELAY;
        assert_eq!(timers.take_due(deadline).unwrap(), Some(first));
        assert_eq!(timers.take_due(deadline).unwrap(), None);
        assert_eq!(timers.next_wake(), Some(deadline + MIN_DELAY));
        assert_eq!(timers.take_due(deadline + MIN_DELAY).unwrap(), Some(second));
        assert!(!timers.cancel(first));
        assert_eq!(timers.next_wake(), None);
    }

    #[test]
    fn enforces_timer_count_delay_cancellation_and_monotonic_input() {
        let start = Instant::now();
        let mut timers = Timers::new(start);
        for delay in [Duration::ZERO, MAX_DELAY + MIN_DELAY] {
            assert_eq!(timers.schedule(start, delay, false), Err(Error::Delay));
        }
        let ids: Vec<_> = (0..MAX_TIMERS)
            .map(|_| timers.schedule(start, MAX_DELAY, false).unwrap())
            .collect();
        assert_eq!(
            timers.schedule(start, MIN_DELAY, false),
            Err(Error::Capacity)
        );
        assert!(timers.cancel(ids[0]));
        let now = start + MIN_DELAY;
        let replacement = timers.schedule(now, MIN_DELAY, false).unwrap();
        assert_ne!(ids[0], replacement);
        assert_eq!(timers.set_paused(start, true), Err(Error::BackwardsTime));
        assert_eq!(timers.take_due(start), Err(Error::BackwardsTime));
        assert_eq!(timers.next_wake(), Some(now + MIN_DELAY));
    }
}
