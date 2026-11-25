//! Fixed multi-message transport fixtures; no scene/source/callback protocol.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const ROUNDS: usize = 128;
pub const FAULT_AFTER: usize = 8;
pub const RESULT_BIT: u32 = 1 << 17;

pub fn read_response(
    output: &mut impl Read,
    session: u64,
    first_nonce: u64,
    ready: &mpsc::SyncSender<()>,
    requests: mpsc::SyncSender<Message>,
    completed: &AtomicUsize,
) -> Result<ProbeReply> {
    let hello = read_frame(output)?.ok_or(MissingFrame("Hello"))?;
    hello.check_context(session, 1, 0)?;
    if hello.body != (Body::Hello { api_version: 1 }) {
        return Err("Invalid roundtrip handshake".into());
    }
    ready.send(())?;
    let mut nonce = first_nonce;
    for round in 1..=ROUNDS as u64 {
        if round > 1 {
            let next = wallpaper_sandbox::random_id()?;
            if next == nonce {
                return Err("Roundtrip nonce collision".into());
            }
            nonce = next;
            // One outstanding Ping; next writer command follows a validated reply.
            requests
                .try_send(envelope(session, round, Body::Ping { nonce }))
                .map_err(|_| "Roundtrip writer queue unavailable")?;
        }
        let pong = read_frame(output)?.ok_or(MissingFrame("roundtrip Pong"))?;
        pong.check_context(session, round + 1, 0)?;
        if pong.body != (Body::Pong { nonce }) {
            return Err("Invalid roundtrip Pong nonce/type".into());
        }
        completed.fetch_add(1, Ordering::SeqCst);
    }
    // Disconnect writer BEFORE the final report: worker requires host input EOF.
    // Error paths also drop this sole sender and release a waiting writer.
    drop(requests);
    let report = read_frame(output)?.ok_or(MissingFrame("roundtrip result"))?;
    report.check_context(session, ROUNDS as u64 + 2, 0)?;
    if report.body
        != (Body::ProbeResult {
            denied: EXPECTED | RESULT_BIT,
        })
    {
        return Err("Invalid roundtrip final result".into());
    }
    require_eof(output)?;
    Ok(ProbeReply::Report(EXPECTED | RESULT_BIT))
}

pub fn worker(
    input: &mut impl Read,
    output: &mut impl Write,
    session: u64,
    first_nonce: u64,
    case: &str,
    denied: u32,
) -> Result<()> {
    let mut nonce = first_nonce;
    for round in 1..=ROUNDS as u64 {
        if round > 1 {
            let request = read_frame(input)?.ok_or(MissingFrame("roundtrip Ping"))?;
            request.check_context(session, round, 0)?;
            let Body::Ping { nonce: next } = request.body else {
                return Err("Expected roundtrip Ping".into());
            };
            if next == nonce || next == 0 {
                return Err("Roundtrip request did not refresh its nonce".into());
            }
            nonce = next;
        }
        if round == FAULT_AFTER as u64 + 1 {
            if case == "ipc-roundtrip-stall" {
                loop {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            if case == "ipc-roundtrip-replay" {
                // Correct current nonce, deliberately stale worker sequence.
                write_frame(output, &envelope(session, round, Body::Pong { nonce }))?;
                output.flush()?;
                loop {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        write_frame(output, &envelope(session, round + 1, Body::Pong { nonce }))?;
        output.flush()?;
    }
    require_eof(input)?;
    write_frame(
        output,
        &envelope(
            session,
            ROUNDS as u64 + 2,
            Body::ProbeResult {
                denied: denied | RESULT_BIT,
            },
        ),
    )?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn full_writer_queue_rejects_without_blocking_reader_shutdown() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &envelope(17, 1, Body::Hello { api_version: 1 })).unwrap();
        write_frame(&mut bytes, &envelope(17, 2, Body::Pong { nonce: 31 })).unwrap();
        let (ready, _ready_receive) = mpsc::sync_channel(1);
        let (requests, _request_receive) = mpsc::sync_channel(1);
        requests
            .send(envelope(17, 999, Body::Ping { nonce: 999 }))
            .unwrap();
        let completed = AtomicUsize::new(0);
        let error = read_response(
            &mut Cursor::new(bytes),
            17,
            31,
            &ready,
            requests,
            &completed,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "Roundtrip writer queue unavailable");
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }
}
