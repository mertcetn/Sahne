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
}

impl Mode {
    pub fn for_case(case: &str) -> Option<Self> {
        match case {
            "scene-transaction" => Some(Self::Typed),
            "scene-script" => Some(Self::Script),
            "scene-timer" => Some(Self::Timer),
            _ => None,
        }
    }
}

fn format_minute(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn resumed_clock_tick() -> Result<u16> {
    use std::time::{SystemTime, UNIX_EPOCH};

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
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(((seconds / 60) % 1440) as u16)
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
    let display_time = if mode == Mode::Timer {
        let utc_minute = resumed_clock_tick()?;
        requests
            .try_send(envelope(session, 2, Body::ClockTick { utc_minute }))
            .map_err(|_| "Clock tick writer queue unavailable")?;
        format_minute(utc_minute)
    } else {
        "12:34".into()
    };
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
    let accepted = read_frame(output)?.ok_or(MissingFrame("scene batch"))?;
    let ack = accept_scene_batch(&mut host, accepted);
    if ack != envelope(session, 1, Body::SceneAck { revision: 1 })
        || !matches!(&host.scene().scene().nodes[0].content, Content::Text { text, .. } if text == &display_time)
    {
        return Err("Scene acknowledgement or committed text mismatch".into());
    }
    let expected_color = if mode != Mode::Typed {
        Color([36, 179, 144, 255])
    } else {
        Color([255; 4])
    };
    if host.scene().scene().nodes[1].content
        != (Content::Rectangle {
            color: expected_color,
        })
    {
        return Err("Committed accent color mismatch".into());
    }
    requests
        .try_send(ack)
        .map_err(|_| "Scene writer queue unavailable")?;
    let committed = host.scene().encode();
    let invalid = read_frame(output)?.ok_or(MissingFrame("invalid scene batch"))?;
    invalid.check_context(session, 2, 1)?;
    let rejection = accept_scene_batch(&mut host, invalid);
    if !matches!(
        rejection.body,
        Body::Reject {
            code: SCENE_REJECT_CODE,
            ..
        }
    ) || !host.is_closed()
        || host.revision() != 1
        || host.scene().encode() != committed
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
            "Clock timer verified: paused dispatch suppressed, one UTC tick acknowledged after resume."
        );
    }
    println!(
        "Scene IPC verified: revision 1 acknowledged, invalid batch rejected, accepted scene preserved."
    );
    Ok(ProbeReply::Report(EXPECTED | RESULT_BIT))
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
    let display_time = if mode == Mode::Timer {
        let tick = read_frame(input)?.ok_or(MissingFrame("clock tick"))?;
        tick.check_context(session, 2, 0)?;
        let Body::ClockTick { utc_minute } = tick.body else {
            return Err("Expected host clock tick".into());
        };
        format_minute(utc_minute)
    } else {
        "12:34".into()
    };
    let text = |value: &str| Edit::Text {
        node: 1,
        value: value.into(),
    };
    send(
        output,
        envelope(
            session,
            1,
            Body::SceneBatch {
                edits: if mode != Mode::Typed {
                    script::clock_edits(&display_time)?
                } else {
                    vec![text(&display_time)]
                },
            },
        ),
    )?;
    let ack = read_frame(input)?.ok_or(MissingFrame("scene acknowledgement"))?;
    if ack != envelope(session, 1, Body::SceneAck { revision: 1 }) {
        return Err("Worker did not receive the expected scene acknowledgement".into());
    }
    send(
        output,
        Message {
            session,
            sequence: 2,
            base_revision: 1,
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
    rejection.check_context(session, 2, 1)?;
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
