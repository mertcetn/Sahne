//! Experimental run-wide retry policy plus fixed actual-process supervision fixtures.
use super::{EXPECTED, MissingFrame, ProbeAttempt, ProbeReply, Result, script};
use std::{
    fs,
    sync::mpsc,
    time::{Duration, Instant},
};

const MAX_ATTEMPTS: u8 = 3;
const BACKOFF: Duration = Duration::from_millis(100);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Completion {
    Healthy,
    Crash,
    Timeout,
    Protocol,
    LaunchFailure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Healthy,
    Exhausted,
    Protocol,
    LaunchFailure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Start {
    Launch(u8),
    Wait(Duration),
    Stopped(Stop),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Ready(Instant),
    Running(u8),
    Stopped(Stop),
}
struct Policy {
    started: u8,
    state: State,
}
impl Policy {
    fn new(now: Instant) -> Self {
        Self {
            started: 0,
            state: State::Ready(now),
        }
    }
    fn start(&mut self, now: Instant) -> std::result::Result<Start, &'static str> {
        match self.state {
            State::Stopped(reason) => Ok(Start::Stopped(reason)),
            State::Running(_) => Err("An attempt is already active"),
            State::Ready(at) if now < at => Ok(Start::Wait(at.duration_since(now))),
            State::Ready(_) => {
                if self.started >= MAX_ATTEMPTS {
                    return Err("Attempt budget invariant violated");
                }
                self.started += 1;
                self.state = State::Running(self.started);
                Ok(Start::Launch(self.started))
            }
        }
    }
    fn complete(
        &mut self,
        attempt: u8,
        completion: Completion,
        now: Instant,
    ) -> std::result::Result<(), &'static str> {
        if self.state != State::Running(attempt) {
            return Err("Stale or duplicate attempt completion");
        }
        self.state = match completion {
            Completion::Healthy => State::Stopped(Stop::Healthy),
            Completion::Protocol => State::Stopped(Stop::Protocol),
            Completion::LaunchFailure => State::Stopped(Stop::LaunchFailure),
            Completion::Crash | Completion::Timeout if self.started >= MAX_ATTEMPTS => {
                State::Stopped(Stop::Exhausted)
            }
            Completion::Crash | Completion::Timeout => State::Ready(now + BACKOFF),
        };
        Ok(())
    }
}

fn eof_failure(error: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
    let eof_kind = |error: &std::io::Error| {
        matches!(
            error.kind(),
            std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::BrokenPipe
        )
    };
    error.downcast_ref::<MissingFrame>().is_some()
        || matches!(error.downcast_ref::<wallpaper_protocol::Error>(),
            Some(wallpaper_protocol::Error::Io(error)) if eof_kind(error))
        || error.downcast_ref::<std::io::Error>().is_some_and(eof_kind)
}
fn classify(attempt: &mut ProbeAttempt) -> Result<Completion> {
    match attempt
        .response
        .take()
        .ok_or("Missing supervisor response")?
    {
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(Completion::Timeout),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("Reader disconnected without a result".into())
        }
        Ok(Ok(ProbeReply::Report(bits)))
            if attempt.handshake && bits == (EXPECTED | script::RESULT_BIT) =>
        {
            match attempt.sandbox.wait(1000)? {
                Some(0) => Ok(Completion::Healthy),
                Some(_) => Ok(Completion::Crash),
                None => Ok(Completion::Timeout),
            }
        }
        Ok(Ok(_)) => Ok(Completion::Protocol),
        Ok(Err(error)) => {
            if !eof_failure(error.as_ref()) {
                return Ok(Completion::Protocol);
            }
            // Missing/partial frames are transient only when an actual failed exit is observed.
            // A clean exit without a complete validated response is a protocol failure.
            match attempt.sandbox.wait(100)? {
                Some(code) if code != 0 => Ok(Completion::Crash),
                _ => Ok(Completion::Protocol),
            }
        }
    }
}

pub(super) fn run(scenario: &str) -> Result<()> {
    let mut policy = Policy::new(Instant::now());
    let mut identities = Vec::with_capacity(MAX_ATTEMPTS as usize);
    let mut previous = None;
    let stopped = loop {
        match policy.start(Instant::now())? {
            Start::Wait(delay) => std::thread::sleep(delay),
            Start::Stopped(reason) => break reason,
            Start::Launch(number) => {
                let (fixture, expected) = match (scenario, number) {
                    ("restart-recover" | "restart-stale", 1) => ("crash", Completion::Crash),
                    ("restart-timeout", 1) => ("hang", Completion::Timeout),
                    ("restart-handshake", 1) => ("no-hello", Completion::Timeout),
                    ("restart-recover" | "restart-timeout" | "restart-handshake", 2) => {
                        ("js-basic", Completion::Healthy)
                    }
                    ("restart-stale", 2) => ("ipc-old-session", Completion::Protocol),
                    ("restart-protocol", 1) => ("ipc-flood", Completion::Protocol),
                    ("restart-exhaust", _) => ("crash", Completion::Crash),
                    _ => return Err("Unexpected attempt in supervisor fixture".into()),
                };
                let (session, nonce) = {
                    let mut attempt = match ProbeAttempt::start(fixture, previous) {
                        Ok(attempt) => attempt,
                        Err(error) => {
                            policy.complete(number, Completion::LaunchFailure, Instant::now())?;
                            return Err(error);
                        }
                    };
                    if identities.iter().any(|&(session, nonce)| {
                        session == attempt.session || nonce == attempt.nonce
                    }) {
                        return Err("Supervisor reused a session or nonce".into());
                    }
                    if fixture == "ipc-old-session"
                        && (!attempt.handshake
                            || !matches!(attempt.response.as_ref(),
                        Some(Ok(Err(error))) if matches!(error.downcast_ref::<wallpaper_protocol::Error>(),
                            Some(wallpaper_protocol::Error::Invalid("unexpected session, sequence or revision")))))
                    {
                        return Err("Old-session fixture did not reach the exact context rejection after Hello".into());
                    }
                    let completion = classify(&mut attempt)?;
                    if fixture == "no-hello"
                        && (attempt.handshake
                            || attempt.channel_wait < Duration::from_secs(3)
                            || attempt.channel_wait >= Duration::from_secs(4))
                    {
                        return Err(
                            "Failed Hello incorrectly received an extra response deadline".into(),
                        );
                    }
                    if completion != expected {
                        return Err(format!("Expected {expected:?}, got {completion:?}").into());
                    }
                    let cleanup = attempt.finish()?;
                    if fs::read(attempt.root.join("denied.txt"))? != b"Sahne sandbox sentinel" {
                        return Err("Supervisor sentinel changed".into());
                    }
                    println!(
                        "Supervisor attempt {number}/{MAX_ATTEMPTS}: {fixture} -> {completion:?}; channel wait={:?}, Job stop/reader join={cleanup:?}; artifacts={}",
                        attempt.channel_wait,
                        attempt.root.display()
                    );
                    (attempt.session, attempt.nonce)
                    // All old process/Job/profile/pipe owners drop before policy admits a replacement.
                };
                identities.push((session, nonce));
                previous = Some(session);
                policy.complete(number, expected, Instant::now())?;
            }
        }
    };
    let expected = match scenario {
        "restart-recover" | "restart-timeout" | "restart-handshake" => (2, Stop::Healthy),
        "restart-exhaust" => (3, Stop::Exhausted),
        "restart-stale" => (2, Stop::Protocol),
        "restart-protocol" => (1, Stop::Protocol),
        _ => return Err("Unknown supervisor scenario".into()),
    };
    if (policy.started, stopped) != expected || identities.len() != usize::from(policy.started) {
        return Err("Supervisor did not stop at the expected boundary".into());
    }
    if policy.start(Instant::now() + Duration::from_secs(3600))? != Start::Stopped(stopped) {
        return Err("Terminal supervisor state reopened".into());
    }
    println!(
        "Probe passed: {scenario}; attempts={}, stop={stopped:?}; fresh sessions, nonces and channels; no further launch.",
        policy.started
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_eof_errors_are_candidates_for_exit_based_retry() {
        assert!(eof_failure(&MissingFrame("Pong")));
        assert!(eof_failure(&wallpaper_protocol::Error::Io(
            std::io::Error::from(std::io::ErrorKind::UnexpectedEof)
        )));
        assert!(eof_failure(&std::io::Error::from(
            std::io::ErrorKind::BrokenPipe
        )));
        assert!(!eof_failure(&super::super::TrailingData));
        assert!(!eof_failure(&wallpaper_protocol::Error::Invalid(
            "unexpected session, sequence or revision"
        )));
        assert!(!eof_failure(&std::io::Error::from(
            std::io::ErrorKind::PermissionDenied
        )));
        let wrong_nonce: Box<dyn std::error::Error + Send + Sync> =
            "Invalid Pong nonce/type".into();
        assert!(!eof_failure(wrong_nonce.as_ref()));
    }
    #[test]
    fn repeated_failures_exhaust_run_wide_budget_without_clock_reset() {
        let origin = Instant::now();
        let mut policy = Policy::new(origin);
        for number in 1..=MAX_ATTEMPTS {
            let now = origin + BACKOFF * u32::from(number - 1);
            assert_eq!(policy.start(now).unwrap(), Start::Launch(number));
            assert!(policy.start(now).is_err());
            policy
                .complete(
                    number,
                    if number % 2 == 0 {
                        Completion::Timeout
                    } else {
                        Completion::Crash
                    },
                    now,
                )
                .unwrap();
            if number < MAX_ATTEMPTS {
                assert_eq!(
                    policy.start(now + BACKOFF / 2).unwrap(),
                    Start::Wait(BACKOFF / 2)
                );
                assert_eq!(
                    policy
                        .start(now + BACKOFF - Duration::from_nanos(1))
                        .unwrap(),
                    Start::Wait(Duration::from_nanos(1))
                );
            }
        }
        for hours in 1..=100 {
            assert_eq!(
                policy
                    .start(origin + Duration::from_secs(hours * 3600))
                    .unwrap(),
                Start::Stopped(Stop::Exhausted)
            );
        }
        assert_eq!(policy.started, MAX_ATTEMPTS);
    }
    #[test]
    fn protocol_and_launch_failures_are_terminal_even_with_budget_left() {
        let now = Instant::now();
        for (completion, stop) in [
            (Completion::Protocol, Stop::Protocol),
            (Completion::LaunchFailure, Stop::LaunchFailure),
            (Completion::Healthy, Stop::Healthy),
        ] {
            let mut policy = Policy::new(now);
            assert_eq!(policy.start(now).unwrap(), Start::Launch(1));
            policy.complete(1, completion, now).unwrap();
            assert_eq!(policy.start(now + BACKOFF).unwrap(), Start::Stopped(stop));
            assert_eq!(policy.started, 1);
        }
    }
    #[test]
    fn late_or_duplicate_completion_cannot_promote_the_next_attempt() {
        let now = Instant::now();
        let mut policy = Policy::new(now);
        assert!(policy.complete(1, Completion::Healthy, now).is_err());
        assert_eq!(policy.start(now).unwrap(), Start::Launch(1));
        policy.complete(1, Completion::Crash, now).unwrap();
        assert!(policy.complete(1, Completion::Healthy, now).is_err());
        assert_eq!(policy.start(now + BACKOFF).unwrap(), Start::Launch(2));
        assert!(
            policy
                .complete(1, Completion::Healthy, now + BACKOFF)
                .is_err()
        );
        assert!(policy.start(now + BACKOFF).is_err());
        policy
            .complete(2, Completion::Healthy, now + BACKOFF)
            .unwrap();
        assert_eq!(
            policy.start(now + BACKOFF).unwrap(),
            Start::Stopped(Stop::Healthy)
        );
    }
}
