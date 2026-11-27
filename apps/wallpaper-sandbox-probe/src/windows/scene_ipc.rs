//! Trusted fixture for host commits and terminal rejection over inherited pipes.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use wallpaper_protocol::{SCENE_REJECT_CODE, SCRIPT_FAILURE_CODE, accept_scene_batch};
use wallpaper_scene::{
    Color, Content, Node, Rect, SCENE_VERSION, Scene, ValidatedScene,
    timer::{MIN_DELAY, Timers},
    transaction::{Edit, HostScene},
};

pub const RESULT_BIT: u32 = 1 << 23;
// Deliberately ungranted target for the embedded scene's terminal rejection test.
const SCRIPT_BACKGROUND_PROBE: u32 = 4;
const PANEL_COLOR: Color = Color([51, 102, 153, 255]);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Typed,
    Script,
    LocalClock,
    LocalScene,
    LocalLifecycle,
    PreviewLifecycle,
    LiveLifecycle,
    LiveLifecycleWatchdog,
    LiveLifecycleExit,
    LiveLifecycleStale,
    LiveLifecycleTruncated,
    Timer,
    RegisteredTimer,
    Utc,
}

impl Mode {
    pub fn for_case(case: &str) -> Option<Self> {
        match case {
            "scene-transaction" => Some(Self::Typed),
            "scene-script" => Some(Self::Script),
            "local-clock-package" => Some(Self::LocalClock),
            "local-script-package" => Some(Self::LocalScene),
            "local-lifecycle-package" => Some(Self::LocalLifecycle),
            "preview-lifecycle-package" => Some(Self::PreviewLifecycle),
            "live-lifecycle-package" => Some(Self::LiveLifecycle),
            "live-lifecycle-watchdog-check" => Some(Self::LiveLifecycleWatchdog),
            "live-lifecycle-exit-check" => Some(Self::LiveLifecycleExit),
            "live-lifecycle-stale-check" => Some(Self::LiveLifecycleStale),
            "live-lifecycle-truncated-check" => Some(Self::LiveLifecycleTruncated),
            "scene-timer" => Some(Self::Timer),
            "scene-timer-registration" => Some(Self::RegisteredTimer),
            "preview-clock-utc" => Some(Self::Utc),
            _ => None,
        }
    }

    fn loads_script(self) -> bool {
        matches!(self, Self::Script | Self::LocalClock) || self.uses_general_bindings()
    }

    fn uses_general_bindings(self) -> bool {
        matches!(
            self,
            Self::LocalScene | Self::LocalLifecycle | Self::PreviewLifecycle
        ) || self.is_live_package()
    }

    fn is_live_package(self) -> bool {
        matches!(
            self,
            Self::LiveLifecycle
                | Self::LiveLifecycleWatchdog
                | Self::LiveLifecycleExit
                | Self::LiveLifecycleStale
                | Self::LiveLifecycleTruncated
        )
    }

    fn updates(self) -> u64 {
        if self.uses_host_timer() { 3 } else { 1 }
    }

    fn uses_host_timer(self) -> bool {
        matches!(self, Self::Timer | Self::RegisteredTimer)
    }
}

fn format_minute(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn resumed_timer(
    delay: Duration,
    mut transition: impl FnMut(bool) -> Result<()>,
) -> Result<(Timers, wallpaper_scene::timer::TimerId)> {
    let start = Instant::now();
    let mut timers = Timers::new(start);
    let id = timers
        .schedule(start, delay, true)
        .map_err(|_| "Could not register timer")?;
    timers
        .set_paused(start, true)
        .map_err(|_| "Could not pause timer")?;
    transition(true)?;
    std::thread::sleep(delay * 4);
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
    transition(false)?;
    let dispatched = Instant::now();
    if timers
        .take_due(dispatched)
        .map_err(|_| "Could not dispatch resumed timer")?
        != Some(id)
        || timers
            .take_due(dispatched)
            .map_err(|_| "Could not check pending timer")?
            .is_some()
        || timers.next_wake() != dispatched.checked_add(delay)
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

pub(super) struct CompiledScriptScene {
    scene: ValidatedScene,
    clock_bindings: Option<(u32, u32)>,
    source: String,
    targets: BTreeMap<String, u32>,
}

fn compiled_script_scene() -> Result<CompiledScriptScene> {
    let package = wallpaper_format::Package::from_entries(vec![
        ("manifest.json".into(), br#"{"formatVersion":1,"apiVersion":1,"id":"probe.scene","name":"Scene binding probe","entry":"index.html","script":"clock.js","permissions":[],"preferredFps":30}"#.to_vec()),
        ("index.html".into(), br#"
            <div style="width:100px;height:100px;background-color:#121212"></div>
            <span id="clock" style="width:100px;height:24px;background-color:#222222">initial</span>
            <span id="accent" style="width:100px;height:24px;background-color:#333333">UTC</span>
            <div id="panel" style="width:100px;height:24px;background-color:#444444"></div>
        "#.to_vec()),
        ("clock.js".into(), include_bytes!("clock_fixture.js").to_vec()),
    ])?;
    let compiled = prepare_script_scene(&package, Mode::Script)?;
    if !compiled.targets.contains_key("panel") {
        return Err("Missing embedded rectangle binding".into());
    }
    if !matches!(
        content(&compiled.scene, SCRIPT_BACKGROUND_PROBE),
        Some(Content::Rectangle { .. })
    ) {
        return Err("Embedded rejection fixture changed its background node".into());
    }
    Ok(compiled)
}

pub(super) fn prepare_script_scene(
    package: &wallpaper_format::Package,
    mode: Mode,
) -> Result<CompiledScriptScene> {
    let (width, height) = if mode.uses_general_bindings() {
        (1920, 1080)
    } else {
        (100, 100)
    };
    let compiled = wallpaper_compiler::compile(
        package,
        wallpaper_compiler::Viewport {
            width,
            height,
            background: Color([0; 4]),
        },
    )?;
    wallpaper_media::validate_assets(compiled.scene(), compiled.images())?;
    let script = compiled
        .script()
        .ok_or("Missing compiled script reference")?;
    let source = package.require_text(script, ".js", "script")?.to_owned();
    let mut targets = compiled.script_targets();
    let clock_bindings = if mode.uses_general_bindings() {
        None
    } else {
        let (&clock, &accent) = targets
            .get("clock")
            .zip(targets.get("accent"))
            .ok_or("Missing compiled clock bindings")?;
        text_state(compiled.scene(), clock)?;
        text_state(compiled.scene(), accent)?;
        targets.retain(|name, node| {
            [clock, accent].contains(node) || (mode == Mode::Script && name == "panel")
        });
        Some((clock, accent))
    };
    Ok(CompiledScriptScene {
        scene: compiled.into_scene(),
        clock_bindings,
        source,
        targets,
    })
}

fn content(scene: &ValidatedScene, id: u32) -> Option<&Content> {
    scene
        .scene()
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| &node.content)
}

fn text_state(scene: &ValidatedScene, id: u32) -> Result<(&str, Color)> {
    match content(scene, id) {
        Some(Content::Text { text, color, .. }) => Ok((text, *color)),
        _ => Err("Expected clock text binding".into()),
    }
}

pub fn read_response(
    output: &mut impl Read,
    session: u64,
    nonce: u64,
    ready: &mpsc::SyncSender<()>,
    requests: mpsc::SyncSender<Message>,
    mode: Mode,
    local_script: Option<CompiledScriptScene>,
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
    let (scene, clock_bindings, program) = if mode.loads_script() {
        let compiled = match mode {
            Mode::LocalClock | Mode::LocalScene | Mode::LocalLifecycle => {
                local_script.ok_or("Missing prepared local package")?
            }
            _ => compiled_script_scene()?,
        };
        (
            compiled.scene,
            compiled.clock_bindings,
            Some((compiled.source, compiled.targets)),
        )
    } else {
        (fixed_scene()?, Some((1, 2)), None)
    };
    let writable = match program.as_ref() {
        Some((_, targets)) => targets.values().copied().collect(),
        None => BTreeSet::from([1, 2]),
    };
    let panel = program
        .as_ref()
        .and_then(|(_, targets)| targets.get("panel"))
        .copied();
    let mut host = HostScene::new(scene, session, &writable)
        .map_err(|_| "Could not initialize fixture host scene")?;
    let clock_state = clock_bindings
        .map(|(clock, accent)| {
            let (_, clock_color) = text_state(host.scene(), clock)?;
            let accent_text = mode
                .loads_script()
                .then(|| text_state(host.scene(), accent).map(|(text, _)| text.to_owned()))
                .transpose()?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((
                clock,
                accent,
                clock_color,
                accent_text,
            ))
        })
        .transpose()?;
    if let Some((source, targets)) = program {
        let mut initial = Vec::new();
        for &node in targets.values() {
            let color = match content(host.scene(), node) {
                Some(Content::Text { text, color, .. }) => {
                    initial.push(Edit::Text {
                        node,
                        value: text.clone(),
                    });
                    *color
                }
                Some(Content::Rectangle { color }) => *color,
                _ => return Err("Unsupported script binding content".into()),
            };
            initial.push(Edit::Color { node, value: color });
        }
        requests
            .try_send(envelope(
                session,
                2,
                Body::SceneProgram {
                    targets,
                    source,
                    initial,
                },
            ))
            .map_err(|_| "Scene program writer queue unavailable")?;
    }
    if mode == Mode::LocalLifecycle {
        let ready = read_script_reply(output, session, 3, 0, "lifecycle readiness")?;
        if ready != envelope(session, 3, Body::LifecycleReady) {
            return Err("Unexpected lifecycle readiness".into());
        }
        for paused in [true, false] {
            notify_lifecycle(output, &requests, session, paused)?;
        }
    }
    let expected_color = if mode != Mode::Typed {
        Color([36, 179, 144, 255])
    } else {
        Color([255; 4])
    };
    let mut timer = match mode {
        Mode::Timer => Some(resumed_timer(MIN_DELAY, |_| Ok(()))?),
        Mode::RegisteredTimer => {
            let registration = read_script_reply(output, session, 3, 0, "interval registration")?;
            registration.check_context(session, 3, 0)?;
            let Body::RegisterInterval { delay_ms } = registration.body else {
                return Err("Unexpected interval registration for clock fixture".into());
            };
            if delay_ms > script::MAX_FIXTURE_INTERVAL_MS {
                return Err("Interval exceeds the fixed probe's duration budget".into());
            }
            let timer = resumed_timer(Duration::from_millis(u64::from(delay_ms)), |paused| {
                notify_lifecycle(output, &requests, session, paused)
            })?;
            println!("Worker interval registered with host: {delay_ms} ms.");
            Some(timer)
        }
        _ => None,
    };
    let updates = mode.updates();
    let mut snapshot = host
        .snapshot_after(None)
        .ok_or("Missing initial snapshot")?;
    let mut published = 0;
    for sequence in 1..=updates {
        let Some((clock, accent, clock_color, accent_text)) = clock_state.as_ref() else {
            let batch = read_script_reply(output, session, sequence, sequence - 1, "scene batch")?;
            let count = match &batch.body {
                Body::SceneBatch { edits } => edits.len(),
                _ => return Err("Expected scene edit batch".into()),
            };
            let ack = accept_scene_batch(&mut host, batch);
            if ack != update_message(session, sequence, Body::SceneAck { revision: sequence }) {
                return Err("Local script scene update rejected".into());
            }
            if let Some(updated) = host.snapshot_after(Some(snapshot.key)) {
                snapshot = updated;
            }
            requests
                .try_send(ack)
                .map_err(|_| "Scene writer queue unavailable")?;
            println!("Local script transaction acknowledged: {count} edits.");
            continue;
        };
        let (clock, accent, clock_color) = (*clock, *accent, *clock_color);
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
        let expected_change = text_state(host.scene(), clock)?.0 != display_time
            || match content(host.scene(), accent) {
                Some(Content::Text { color, .. } | Content::Rectangle { color }) => {
                    *color != expected_color
                }
                _ => return Err("Missing clock accent content".into()),
            }
            || panel.is_some_and(|id| {
                content(host.scene(), id) != Some(&Content::Rectangle { color: PANEL_COLOR })
            });
        let accepted = read_script_reply(output, session, sequence, sequence - 1, "scene batch")?;
        let ack = accept_scene_batch(&mut host, accepted);
        let accent_matches = match content(host.scene(), accent) {
            Some(Content::Text { text, color, .. }) if mode.loads_script() => {
                accent_text.as_deref() == Some(text.as_str()) && *color == expected_color
            }
            Some(Content::Rectangle { color }) if !mode.loads_script() => *color == expected_color,
            _ => false,
        };
        if ack != update_message(session, sequence, Body::SceneAck { revision: sequence })
            || text_state(host.scene(), clock)? != (display_time.as_str(), clock_color)
            || !accent_matches
            || panel.is_some_and(|id| {
                content(host.scene(), id) != Some(&Content::Rectangle { color: PANEL_COLOR })
            })
        {
            return Err("Scene acknowledgement or committed content mismatch".into());
        }
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
        requests
            .try_send(ack)
            .map_err(|_| "Scene writer queue unavailable")?;
    }
    if mode == Mode::RegisteredTimer {
        let (mut timers, id) = timer.take().ok_or("Missing registered interval")?;
        let sequence = updates + 1;
        let cancel = read_frame(output)?.ok_or(MissingFrame("interval cancellation"))?;
        if cancel != update_message(session, sequence, Body::CancelInterval) {
            return Err("Unexpected interval cancellation".into());
        }
        let next_wake = timers
            .next_wake()
            .ok_or("Missing interval deadline before cancellation")?;
        if !timers.cancel(id)
            || timers.next_wake().is_some()
            || timers
                .take_due(next_wake)
                .map_err(|_| "Could not check cancelled interval")?
                .is_some()
        {
            return Err("Cancelled interval remained eligible for dispatch".into());
        }
        requests
            .try_send(update_message(session, sequence, Body::IntervalCancelled))
            .map_err(|_| "Interval cancellation writer queue unavailable")?;
        println!("Worker interval cancelled with no dispatch at its former deadline.");
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
    if mode.uses_host_timer() {
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
    if mode.uses_general_bindings() && snapshot.scene.scene().resources.is_empty() {
        return Ok(ProbeReply::SceneSnapshot {
            bits,
            scene: committed,
        });
    }
    Ok(ProbeReply::Report(bits))
}

fn fixed_scene() -> Result<ValidatedScene> {
    Ok(Scene {
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
    .validate()?)
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
    let registered_clock = if mode == Mode::RegisteredTimer {
        let (callback, delay_ms) =
            script_result(output, session, 3, 0, script::RegisteredClock::new())?;
        send(
            output,
            envelope(session, 3, Body::RegisterInterval { delay_ms }),
        )?;
        deliver_lifecycle(input, output, session, |paused| {
            callback.callbacks.lifecycle(paused)?;
            callback.check_controls()?;
            Ok(())
        })?;
        Some(callback)
    } else {
        None
    };
    let (mut bindings, source) = if mode.loads_script() {
        let program = read_frame(input)?.ok_or(MissingFrame("scene program"))?;
        program.check_context(session, 2, 0)?;
        let Body::SceneProgram {
            targets,
            source,
            initial,
        } = program.body
        else {
            return Err("Expected host scene program".into());
        };
        (
            script::bindings_from_initial(targets, initial)?,
            Some(source),
        )
    } else {
        (
            vec![
                script::Binding {
                    name: "clock".into(),
                    node: 1,
                    text: Some("initial".into()),
                    color: Color([255; 4]),
                },
                script::Binding {
                    name: "accent".into(),
                    node: 2,
                    text: None,
                    color: Color([255; 4]),
                },
            ],
            None,
        )
    };
    let (local_callbacks, package_ticks) = if matches!(
        mode,
        Mode::LocalLifecycle | Mode::PreviewLifecycle
    ) || mode.is_live_package()
    {
        let source = source.as_deref().ok_or("Missing local lifecycle source")?;
        let (callbacks, timer) = script_result(
            output,
            session,
            3,
            0,
            if mode == Mode::PreviewLifecycle || mode.is_live_package() {
                script::RetainedCallbacks::from_package_source(source, mode.is_live_package())
            } else {
                script::RetainedCallbacks::from_source(source).map(|callbacks| (callbacks, None))
            },
        )?;
        if let Some(timer) = &timer {
            send(
                output,
                envelope(
                    session,
                    3,
                    Body::PackageTimer {
                        delay_ms: timer.delay_ms,
                        ticks: timer.ticks,
                    },
                ),
            )?;
        } else {
            send(output, envelope(session, 3, Body::LifecycleReady))?;
        }
        if !mode.is_live_package() {
            deliver_lifecycle(input, output, session, |paused| callbacks.lifecycle(paused))?;
        }
        (Some(callbacks), timer.map(|timer| u64::from(timer.ticks)))
    } else {
        (None, None)
    };
    let clock = if mode.uses_general_bindings() {
        bindings
            .first()
            .ok_or("Missing local script bindings")?
            .node
    } else {
        bindings
            .iter()
            .find(|binding| binding.name == "clock" && binding.text.is_some())
            .ok_or("Missing clock text binding")?
            .node
    };
    let text = |value: &str| Edit::Text {
        node: clock,
        value: value.into(),
    };
    let updates = package_ticks.unwrap_or_else(|| mode.updates());
    let mut sequence = 1u64;
    let utc_callbacks = (mode == Mode::Utc)
        .then(script::RetainedCallbacks::utc_clock)
        .transpose()?;
    let mut lifecycle_sequence = 1u64;
    let mut utc_paused = false;
    let mut package_paused = false;
    let mut package_lifecycle_sequence = 4;
    let mut acknowledged_fixture_batch = None;
    while mode == Mode::Utc || mode.is_live_package() || sequence <= updates {
        if package_ticks.is_some() {
            loop {
                let request = read_frame(input)?.ok_or(MissingFrame("package tick"))?;
                if mode.is_live_package()
                    && let Body::Lifecycle { paused } = &request.body
                {
                    request.check_context(session, package_lifecycle_sequence, sequence - 1)?;
                    if *paused == package_paused {
                        return Err("Duplicate package lifecycle transition".into());
                    }
                    let callbacks = local_callbacks
                        .as_ref()
                        .ok_or("Missing package callbacks")?;
                    script_result(
                        output,
                        session,
                        package_lifecycle_sequence,
                        sequence - 1,
                        callbacks.lifecycle(*paused),
                    )?;
                    send(
                        output,
                        Message {
                            body: Body::LifecycleAck { paused: *paused },
                            ..request
                        },
                    )?;
                    package_paused = *paused;
                    package_lifecycle_sequence = package_lifecycle_sequence
                        .checked_add(1)
                        .ok_or("Package lifecycle sequence exhausted")?;
                } else {
                    request.check_context(
                        session,
                        sequence
                            .checked_add(5)
                            .ok_or("Package tick sequence exhausted")?,
                        sequence - 1,
                    )?;
                    if request.body != Body::PackageTick || package_paused {
                        return Err("Expected host package tick".into());
                    }
                    break;
                }
            }
        }
        if sequence == 2 {
            match mode {
                Mode::LiveLifecycleWatchdog => {
                    // Deliberately unresponsive native test worker, independent of QuickJS limits.
                    std::thread::sleep(Duration::from_secs(15));
                    return Err("Watchdog fixture reached its fallback deadline".into());
                }
                Mode::LiveLifecycleExit => return Ok(()),
                Mode::LiveLifecycleStale => {
                    send(
                        output,
                        acknowledged_fixture_batch
                            .take()
                            .ok_or("Missing fixture batch")?,
                    )?;
                    return Ok(());
                }
                Mode::LiveLifecycleTruncated => {
                    let mut bytes = Vec::new();
                    write_frame(
                        &mut bytes,
                        &update_message(
                            session,
                            sequence,
                            Body::SceneBatch {
                                edits: vec![Edit::Color {
                                    node: clock,
                                    value: PANEL_COLOR,
                                }],
                            },
                        ),
                    )?;
                    bytes.pop();
                    output.write_all(&bytes)?;
                    output.flush()?;
                    return Ok(());
                }
                _ => {}
            }
        }
        let display_time = if mode.uses_host_timer() || mode == Mode::Utc {
            if sequence > 1 {
                send(output, update_message(session, sequence, Body::ClockReady))?;
            }
            let tick = loop {
                let message = read_frame(input)?.ok_or(MissingFrame("clock tick"))?;
                if let (Some(callbacks), Body::Lifecycle { paused }) =
                    (&utc_callbacks, &message.body)
                {
                    message.check_context(session, lifecycle_sequence, sequence - 1)?;
                    if *paused == utc_paused {
                        return Err("Duplicate clock lifecycle transition".into());
                    }
                    script_result(
                        output,
                        session,
                        lifecycle_sequence,
                        sequence - 1,
                        callbacks.lifecycle(*paused),
                    )?;
                    utc_paused = *paused;
                    send(
                        output,
                        Message {
                            body: Body::LifecycleAck { paused: *paused },
                            ..message
                        },
                    )?;
                    lifecycle_sequence = lifecycle_sequence
                        .checked_add(1)
                        .ok_or("Clock lifecycle sequence exhausted")?;
                } else {
                    break message;
                }
            };
            tick.check_context(session, sequence, sequence - 1)?;
            let Body::ClockTick { utc_minute } = tick.body else {
                return Err("Expected host clock tick".into());
            };
            format_minute(utc_minute)
        } else if mode.uses_general_bindings() {
            String::new()
        } else {
            "12:34".into()
        };
        let edits = if let Some(callback) = &registered_clock {
            let edits = script_result(
                output,
                session,
                sequence,
                sequence - 1,
                callback.edits(&bindings, &display_time),
            )?;
            if callback.cancelled() != (sequence == updates) {
                return Err("JavaScript interval did not cancel on its third callback".into());
            }
            edits
        } else if let Some(callbacks) = local_callbacks.as_ref().or(utc_callbacks.as_ref()) {
            script_result(
                output,
                session,
                sequence,
                sequence - 1,
                callbacks.edits(&bindings, &display_time),
            )?
        } else if mode != Mode::Typed {
            script_result(
                output,
                session,
                sequence,
                sequence - 1,
                script::scene_edits(&bindings, &display_time, source.as_deref()),
            )?
        } else {
            vec![text(&display_time)]
        };
        let batch = update_message(session, sequence, Body::SceneBatch { edits });
        send(output, &batch)?;
        let ack = read_frame(input)?.ok_or(MissingFrame("scene acknowledgement"))?;
        if ack != update_message(session, sequence, Body::SceneAck { revision: sequence }) {
            return Err("Worker did not receive the expected scene acknowledgement".into());
        }
        if mode == Mode::LiveLifecycleStale && sequence == 1 {
            acknowledged_fixture_batch = Some(batch.clone());
        }
        if let Body::SceneBatch { edits } = batch.body {
            script::apply_accepted_edits(&mut bindings, edits);
        }
        sequence = sequence.checked_add(1).ok_or("Clock sequence exhausted")?;
    }
    if registered_clock.is_some_and(|callback| callback.cancelled()) {
        send(
            output,
            update_message(session, sequence, Body::CancelInterval),
        )?;
        let reply =
            read_frame(input)?.ok_or(MissingFrame("interval cancellation acknowledgement"))?;
        if reply != update_message(session, sequence, Body::IntervalCancelled) {
            return Err("Unexpected interval cancellation acknowledgement".into());
        }
    }
    let terminal_edit = if mode == Mode::Script || mode.uses_general_bindings() {
        Edit::Color {
            node: if mode == Mode::Script {
                SCRIPT_BACKGROUND_PROBE
            } else {
                0
            },
            value: Color([255; 4]),
        }
    } else {
        text("invalid\0text")
    };
    send(
        output,
        Message {
            session,
            sequence: updates + 1,
            base_revision: updates,
            body: Body::SceneBatch {
                edits: vec![
                    Edit::Color {
                        node: clock,
                        value: Color([0; 4]),
                    },
                    terminal_edit,
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

fn lifecycle_sequence(paused: bool) -> u64 {
    if paused { 4 } else { 5 }
}

fn script_result<T>(
    output: &mut impl Write,
    session: u64,
    sequence: u64,
    base_revision: u64,
    result: Result<T>,
) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(_) => {
            send(
                output,
                Message {
                    session,
                    sequence,
                    base_revision,
                    body: Body::Reject {
                        code: SCRIPT_FAILURE_CODE,
                        message: "Script execution failed".into(),
                    },
                },
            )?;
            Err("Script execution failed".into())
        }
    }
}

fn read_script_reply(
    output: &mut impl Read,
    session: u64,
    sequence: u64,
    base_revision: u64,
    phase: &'static str,
) -> Result<Message> {
    let reply = read_frame(output)?.ok_or(MissingFrame(phase))?;
    if matches!(
        &reply.body,
        Body::Reject {
            code: SCRIPT_FAILURE_CODE,
            ..
        }
    ) {
        reply.check_context(session, sequence, base_revision)?;
        return Err(format!("Worker script failed during {phase}").into());
    }
    Ok(reply)
}

fn notify_lifecycle(
    output: &mut impl Read,
    requests: &mpsc::SyncSender<Message>,
    session: u64,
    paused: bool,
) -> Result<()> {
    let sequence = lifecycle_sequence(paused);
    requests
        .try_send(envelope(session, sequence, Body::Lifecycle { paused }))
        .map_err(|_| "Lifecycle writer queue unavailable")?;
    let ack = read_script_reply(output, session, sequence, 0, "lifecycle acknowledgement")?;
    if ack != envelope(session, sequence, Body::LifecycleAck { paused }) {
        return Err("Unexpected lifecycle acknowledgement".into());
    }
    let event = if paused { "pause" } else { "resume" };
    println!("JavaScript lifecycle acknowledged: {event}.");
    Ok(())
}

fn deliver_lifecycle(
    input: &mut impl Read,
    output: &mut impl Write,
    session: u64,
    mut callback: impl FnMut(bool) -> Result<()>,
) -> Result<()> {
    for paused in [true, false] {
        let sequence = lifecycle_sequence(paused);
        let event = read_frame(input)?.ok_or(MissingFrame("lifecycle event"))?;
        if event != envelope(session, sequence, Body::Lifecycle { paused }) {
            return Err("Unexpected lifecycle event".into());
        }
        script_result(output, session, sequence, 0, callback(paused))?;
        send(
            output,
            envelope(session, sequence, Body::LifecycleAck { paused }),
        )?;
    }
    Ok(())
}

fn send(output: &mut impl Write, message: impl std::borrow::Borrow<Message>) -> Result<()> {
    write_frame(output, message.borrow())?;
    output.flush()?;
    Ok(())
}
