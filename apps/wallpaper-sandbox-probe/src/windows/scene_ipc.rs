//! Trusted fixture for host commits and terminal rejection over inherited pipes.
use super::*;
use std::collections::BTreeSet;
use wallpaper_protocol::{SCENE_REJECT_CODE, accept_scene_batch};
use wallpaper_scene::{
    Color, Content, Node, Rect, SCENE_VERSION, Scene,
    timer::{MIN_DELAY, Timers},
    transaction::{Edit, HostScene},
};

pub const RESULT_BIT: u32 = 1 << 23;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Typed,
    Script,
    Timer,
    Utc,
}

impl Mode {
    pub fn for_case(case: &str) -> Option<Self> {
        match case {
            "scene-transaction" => Some(Self::Typed),
            "scene-script" => Some(Self::Script),
            "scene-timer" => Some(Self::Timer),
            "preview-clock-utc" => Some(Self::Utc),
            _ => None,
        }
    }

    fn updates(self) -> u64 {
        if self == Self::Timer { 3 } else { 1 }
    }
}

fn format_minute(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn resumed_timer() -> Result<(Timers, wallpaper_scene::timer::TimerId)> {
    let start = Instant::now();
    let mut timers = Timers::new(start);
    let id = timers
        .schedule(start, MIN_DELAY, true)
        .map_err(|_| "Could not register timer")?;
    timers
        .set_paused(start, true)
        .map_err(|_| "Could not pause timer")?;
    std::thread::sleep(MIN_DELAY * 4);
    let resumed = Instant::now();
    if timers.next_wake().is_some()
        || timers
            .take_due(resumed)
            .map_err(|_| "Could not check paused timer")?
            .is_some()
    {
        return Err("Paused timer dispatched a callback".into());
    }
    timers
        .set_paused(resumed, false)
        .map_err(|_| "Could not resume timer")?;
    if timers
        .take_due(resumed)
        .map_err(|_| "Could not dispatch resumed timer")?
        != Some(id)
        || timers
            .take_due(resumed)
            .map_err(|_| "Could not check pending timer")?
            .is_some()
        || timers.next_wake() != resumed.checked_add(MIN_DELAY)
    {
        return Err("Timer did not dispatch exactly once after resume".into());
    }
    Ok((timers, id))
}

fn utc_minute() -> Result<u16> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(((seconds / 60) % 1440) as u16)
}

fn wait_for_next_tick(timers: &mut Timers, id: wallpaper_scene::timer::TimerId) -> Result<()> {
    let deadline = timers.next_wake().ok_or("Missing next clock deadline")?;
    let now = Instant::now();
    if deadline > now {
        std::thread::sleep(deadline.duration_since(now));
    }
    if timers
        .take_due(Instant::now())
        .map_err(|_| "Clock timer failed")?
        != Some(id)
    {
        return Err("Clock timer did not become due".into());
    }
    Ok(())
}

fn update_message(session: u64, sequence: u64, body: Body) -> Message {
    Message {
        session,
        sequence,
        base_revision: sequence - 1,
        body,
    }
}

pub fn read_response(
    output: &mut impl Read,
    session: u64,
    nonce: u64,
    ready: &mpsc::SyncSender<()>,
    requests: mpsc::SyncSender<Message>,
    mode: Mode,
) -> Result<ProbeReply> {
    let hello = read_frame(output)?.ok_or(MissingFrame("scene Hello"))?;
    if hello != envelope(session, 1, Body::Hello { api_version: 1 }) {
        return Err("Invalid scene handshake".into());
    }
    ready.send(())?;
    let pong = read_frame(output)?.ok_or(MissingFrame("scene Pong"))?;
    if pong != envelope(session, 2, Body::Pong { nonce }) {
        return Err("Invalid scene Pong".into());
    }
    let scene = Scene {
        version: SCENE_VERSION,
        width: 100,
        height: 100,
        background: Color([0; 4]),
        resources: vec![],
        nodes: vec![
            Node {
                id: 1,
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                content: Content::Text {
                    text: "initial".into(),
                    font_size: 16.0,
                    color: Color([255; 4]),
                },
            },
            Node {
                id: 2,
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                content: Content::Rectangle {
                    color: Color([255; 4]),
                },
            },
        ],
    }
    .validate()?;
    let mut host = HostScene::new(scene, session, &BTreeSet::from([1, 2]))
        .map_err(|_| "Could not initialize fixture host scene")?;
    let expected_color = if mode != Mode::Typed {
        Color([36, 179, 144, 255])
    } else {
        Color([255; 4])
    };
    let mut timer = if mode == Mode::Timer {
        Some(resumed_timer()?)
    } else {
        None
    };
    let updates = mode.updates();
    let mut snapshot = host
        .snapshot_after(None)
        .ok_or("Missing initial snapshot")?;
    let mut previous_display = "initial".to_owned();
    let mut published = 0;
    for sequence in 1..=updates {
        let display_time = if let Some((timers, id)) = timer.as_mut() {
            if sequence > 1 {
                let received = read_frame(output)?.ok_or(MissingFrame("clock readiness"))?;
                if received != update_message(session, sequence, Body::ClockReady) {
                    return Err("Unexpected clock readiness".into());
                }
                wait_for_next_tick(timers, *id)?;
            }
            let utc_minute = utc_minute()?;
            requests
                .try_send(update_message(
                    session,
                    sequence,
                    Body::ClockTick { utc_minute },
                ))
                .map_err(|_| "Clock tick writer queue unavailable")?;
            format_minute(utc_minute)
        } else {
            "12:34".into()
        };
        let accepted = read_frame(output)?.ok_or(MissingFrame("scene batch"))?;
        let ack = accept_scene_batch(&mut host, accepted);
        if ack != update_message(session, sequence, Body::SceneAck { revision: sequence })
            || !matches!(&host.scene().scene().nodes[0].content, Content::Text { text, .. } if text == &display_time)
            || host.scene().scene().nodes[1].content
                != (Content::Rectangle {
                    color: expected_color,
                })
        {
            return Err("Scene acknowledgement or committed content mismatch".into());
        }
        let expected_change = display_time != previous_display;
        let updated = host.snapshot_after(Some(snapshot.key));
        if updated.is_some() != expected_change {
            return Err("Unexpected presentation update for clock text".into());
        }
        if let Some(updated) = updated {
            if updated.scene.as_ref() != host.scene() {
                return Err("Published snapshot differs from committed scene".into());
            }
            snapshot = updated;
            published += 1;
        }
        previous_display = display_time;
        requests
            .try_send(ack)
            .map_err(|_| "Scene writer queue unavailable")?;
    }
    let committed = host.scene().encode();
    let invalid = read_frame(output)?.ok_or(MissingFrame("invalid scene batch"))?;
    invalid.check_context(session, updates + 1, updates)?;
    let rejection = accept_scene_batch(&mut host, invalid);
    if !matches!(
        rejection.body,
        Body::Reject {
            code: SCENE_REJECT_CODE,
            ..
        }
    ) || !host.is_closed()
        || host.revision() != updates
        || host.scene().encode() != committed
        || host.snapshot_after(Some(snapshot.key)).is_some()
    {
        return Err("Rejected scene transaction changed host state".into());
    }
    requests
        .try_send(rejection)
        .map_err(|_| "Scene writer queue unavailable")?;
    drop(requests);
    let report = read_frame(output)?.ok_or(MissingFrame("scene result"))?;
    if report
        != envelope(
            session,
            3,
            Body::ProbeResult {
                denied: EXPECTED | RESULT_BIT,
            },
        )
    {
        return Err("Invalid scene transaction result".into());
    }
    require_eof(output)?;
    if mode == Mode::Timer {
        println!(
            "Clock timer verified: paused dispatch suppressed, {updates} sequential UTC ticks acknowledged after resume."
        );
        println!("Clock snapshots verified: {published} content changes for {updates} revisions.");
    }
    println!(
        "Scene IPC verified: revision {updates} acknowledged, invalid batch rejected, accepted scene preserved."
    );
    let bits = EXPECTED | RESULT_BIT;
    if mode == Mode::Timer {
        let Content::Text { text, .. } = &snapshot.scene.scene().nodes[0].content else {
            return Err("Missing accepted clock text".into());
        };
        let text = text
            .as_bytes()
            .try_into()
            .map_err(|_| "Invalid clock snapshot length")?;
        return Ok(ProbeReply::ClockSnapshot { bits, text });
    }
    Ok(ProbeReply::Report(bits))
}

pub fn worker(
    input: &mut impl Read,
    output: &mut impl Write,
    session: u64,
    nonce: u64,
    denied: u32,
    mode: Mode,
) -> Result<()> {
    send(output, envelope(session, 2, Body::Pong { nonce }))?;
    let text = |value: &str| Edit::Text {
        node: 1,
        value: value.into(),
    };
    let updates = mode.updates();
    let mut previous = "initial".to_owned();
    let mut sequence = 1u64;
    while mode == Mode::Utc || sequence <= updates {
        let display_time = if matches!(mode, Mode::Timer | Mode::Utc) {
            if sequence > 1 {
                send(output, update_message(session, sequence, Body::ClockReady))?;
            }
            let tick = read_frame(input)?.ok_or(MissingFrame("clock tick"))?;
            tick.check_context(session, sequence, sequence - 1)?;
            let Body::ClockTick { utc_minute } = tick.body else {
                return Err("Expected host clock tick".into());
            };
            format_minute(utc_minute)
        } else {
            "12:34".into()
        };
        let edits = if mode != Mode::Typed {
            script::clock_edits(&previous, &display_time)?
        } else {
            vec![text(&display_time)]
        };
        send(
            output,
            update_message(session, sequence, Body::SceneBatch { edits }),
        )?;
        let ack = read_frame(input)?.ok_or(MissingFrame("scene acknowledgement"))?;
        if ack != update_message(session, sequence, Body::SceneAck { revision: sequence }) {
            return Err("Worker did not receive the expected scene acknowledgement".into());
        }
        previous = display_time;
        sequence = sequence.checked_add(1).ok_or("Clock sequence exhausted")?;
    }
    send(
        output,
        Message {
            session,
            sequence: updates + 1,
            base_revision: updates,
            body: Body::SceneBatch {
                edits: vec![
                    Edit::Color {
                        node: 1,
                        value: Color([0; 4]),
                    },
                    text("invalid\0text"),
                ],
            },
        },
    )?;
    let rejection = read_frame(input)?.ok_or(MissingFrame("scene rejection"))?;
    rejection.check_context(session, updates + 1, updates)?;
    if !matches!(
        rejection.body,
        Body::Reject {
            code: SCENE_REJECT_CODE,
            ..
        }
    ) {
        return Err("Worker did not receive the expected terminal rejection".into());
    }
    require_eof(input)?;
    send(
        output,
        envelope(
            session,
            3,
            Body::ProbeResult {
                denied: denied | RESULT_BIT,
            },
        ),
    )
}

fn send(output: &mut impl Write, message: Message) -> Result<()> {
    write_frame(output, &message)?;
    output.flush()?;
    Ok(())
}
