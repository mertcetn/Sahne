use super::*;
use crate::{
    Color, Content, Rect,
    hierarchy::{Hierarchy, Layer, Transform},
};

fn group(id: u32, parent: u32, scale: f32) -> Layer {
    Layer {
        id,
        parent,
        bounds: Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        },
        content: LayerContent::Group {
            transform: Transform {
                scale: [scale; 2],
                ..Default::default()
            },
            opacity: 1.0,
            clip: true,
        },
    }
}
fn scene_with(layers: Vec<Layer>, width: u32, height: u32) -> ValidatedHierarchy {
    Hierarchy {
        version: hierarchy::VERSION,
        width,
        height,
        background: Color([0, 0, 0, 255]),
        resources: vec![],
        layers,
    }
    .validate()
    .unwrap()
}
fn base() -> ValidatedHierarchy {
    scene_with(
        vec![
            group(1, 0, 1.0),
            Layer {
                id: 2,
                parent: 1,
                bounds: Rect {
                    x: 5.0,
                    y: 6.0,
                    width: 10.0,
                    height: 20.0,
                },
                content: LayerContent::Draw(Content::Rectangle {
                    color: Color([255; 4]),
                }),
            },
        ],
        640,
        480,
    )
}
fn track(target: u32, property: Property, start: f32, end: f32) -> Track {
    Track {
        target,
        property,
        keys: vec![
            Key {
                time_ms: 0,
                value: start,
            },
            Key {
                time_ms: 1000,
                value: end,
            },
        ],
    }
}
fn program(tracks: Vec<Track>) -> Animation {
    Animation {
        version: VERSION,
        duration_ms: 1000,
        delay_ms: 0,
        iterations: Iterations::Count(1),
        alternate: false,
        tracks,
    }
}
fn value(sample: &Sample, property: Property) -> f32 {
    let LayerContent::Group {
        transform, opacity, ..
    } = sample.scene.scene().layers[0].content
    else {
        panic!()
    };
    match property {
        Property::TranslateX => transform.translate[0],
        Property::TranslateY => transform.translate[1],
        Property::ScaleX => transform.scale[0],
        Property::ScaleY => transform.scale[1],
        Property::Opacity => opacity,
    }
}
#[test]
fn independent_midpoint_geometry_and_opacity_preserve_group_semantics() {
    let animation = program(vec![
        track(1, Property::TranslateX, -10.0, 30.0),
        track(1, Property::TranslateY, 0.0, 40.0),
        track(1, Property::ScaleX, 1.0, 3.0),
        track(1, Property::ScaleY, 1.0, 2.0),
        track(1, Property::Opacity, 0.2, 0.8),
    ])
    .validate(base())
    .unwrap();
    let sample = animation.sample(Duration::from_millis(500)).unwrap();
    assert!(sample.active);
    assert_eq!(
        sample.scene.resolved()[1].bounds,
        Rect {
            x: 30.0,
            y: 49.0,
            width: 20.0,
            height: 30.0
        }
    );
    assert!((value(&sample, Property::Opacity) - 0.5).abs() < 0.000001);
    assert_eq!(
        sample.scene.scene().layers[1].content,
        animation.base().scene().layers[1].content
    );
    assert_eq!(animation.base().resolved()[1].bounds.x, 15.0);
}
#[test]
fn multiple_keys_submillisecond_sampling_and_end_fill() {
    let mut item = track(1, Property::TranslateX, 0.0, 10.0);
    item.keys.insert(
        1,
        Key {
            time_ms: 250,
            value: 20.0,
        },
    );
    let animation = program(vec![item]).validate(base()).unwrap();
    assert_eq!(
        value(
            &animation.sample(Duration::from_millis(125)).unwrap(),
            Property::TranslateX
        ),
        10.0
    );
    assert_eq!(
        value(
            &animation.sample(Duration::from_millis(250)).unwrap(),
            Property::TranslateX
        ),
        20.0
    );
    assert_eq!(
        value(
            &animation.sample(Duration::from_micros(125_500)).unwrap(),
            Property::TranslateX
        ),
        10.04
    );
    for time in [Duration::from_secs(1), Duration::MAX] {
        let sample = animation.sample(time).unwrap();
        assert!(!sample.active);
        assert_eq!(value(&sample, Property::TranslateX), 10.0);
    }
}
#[test]
fn delay_repeat_boundaries_and_alternate_final_pose() {
    let mut source = program(vec![track(1, Property::TranslateX, 0.0, 100.0)]);
    source.delay_ms = 500;
    source.iterations = Iterations::Count(2);
    source.alternate = true;
    let animation = source.validate(base()).unwrap();
    for (ms, expected, active) in [
        (0, 0.0, true),
        (499, 0.0, true),
        (500, 0.0, true),
        (1000, 50.0, true),
        (1500, 100.0, true),
        (2000, 50.0, true),
        (2500, 0.0, false),
        (10_000, 0.0, false),
    ] {
        let sample = animation.sample(Duration::from_millis(ms)).unwrap();
        assert_eq!(sample.active, active);
        assert_eq!(value(&sample, Property::TranslateX), expected);
    }
    let mut source = program(vec![track(1, Property::TranslateX, 0.0, 100.0)]);
    source.iterations = Iterations::Count(3);
    source.alternate = true;
    assert_eq!(
        value(
            &source
                .validate(base())
                .unwrap()
                .sample(Duration::MAX)
                .unwrap(),
            Property::TranslateX
        ),
        100.0
    );
}
#[test]
fn ordinary_loop_restarts_and_large_infinite_times_are_constant_work() {
    let mut source = program(vec![track(1, Property::TranslateX, 0.0, 100.0)]);
    source.iterations = Iterations::Forever;
    let animation = source.validate(base()).unwrap();
    assert_eq!(
        value(
            &animation.sample(Duration::from_secs(1)).unwrap(),
            Property::TranslateX
        ),
        0.0
    );
    let sample = animation.sample(Duration::MAX).unwrap();
    assert!(sample.active);
    assert!((value(&sample, Property::TranslateX) - 99.999_99).abs() < 0.0001);
}
#[test]
fn typed_version_and_timing_limits_are_enforced() {
    for mutation in 0..6 {
        let mut source = program(vec![track(1, Property::Opacity, 0.0, 1.0)]);
        match mutation {
            0 => source.version = 0,
            1 => source.duration_ms = 0,
            2 => source.duration_ms = MAX_DURATION_MS + 1,
            3 => source.delay_ms = MAX_DURATION_MS + 1,
            4 => source.iterations = Iterations::Count(0),
            _ => source.iterations = Iterations::Count(MAX_ITERATIONS + 1),
        }
        assert!(source.validate(base()).is_err());
    }
}
#[test]
fn rejects_missing_draw_and_duplicate_property_targets() {
    for target in [0, 2, 3, u32::MAX] {
        assert_eq!(
            program(vec![track(target, Property::Opacity, 0.0, 1.0)])
                .validate(base())
                .unwrap_err()
                .code,
            Code::Target
        );
    }
    assert_eq!(
        program(vec![
            track(1, Property::ScaleX, 0.0, 1.0),
            track(1, Property::ScaleX, 1.0, 2.0)
        ])
        .validate(base())
        .unwrap_err()
        .code,
        Code::Duplicate
    );
}
#[test]
fn rejects_nonfinite_and_out_of_range_values_for_every_property() {
    for (property, bad) in [
        (Property::TranslateX, 32769.0),
        (Property::TranslateY, -32769.0),
        (Property::ScaleX, -0.1),
        (Property::ScaleY, 16.01),
        (Property::Opacity, 1.01),
        (Property::Opacity, -0.01),
    ] {
        assert_eq!(
            program(vec![track(1, property, 1.0, bad)])
                .validate(base())
                .unwrap_err()
                .code,
            Code::Value
        );
    }
    for property in [
        Property::TranslateX,
        Property::TranslateY,
        Property::ScaleX,
        Property::ScaleY,
        Property::Opacity,
    ] {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                program(vec![track(1, property, 1.0, bad)])
                    .validate(base())
                    .unwrap_err()
                    .code,
                Code::Value
            );
        }
    }
}
#[test]
fn requires_complete_strictly_ordered_key_times() {
    for times in [
        vec![1, 1000],
        vec![0, 999],
        vec![0, 500, 500, 1000],
        vec![0, 900, 500, 1000],
        vec![0, 1001, 1000],
    ] {
        let mut item = track(1, Property::Opacity, 0.0, 1.0);
        item.keys = times
            .into_iter()
            .map(|time_ms| Key {
                time_ms,
                value: 0.5,
            })
            .collect();
        assert_eq!(
            program(vec![item]).validate(base()).unwrap_err().code,
            Code::KeyTime
        );
    }
}
#[test]
fn track_and_key_allocation_budgets_reject_before_binding_or_sampling() {
    for tracks in [
        vec![],
        vec![track(1, Property::Opacity, 0.0, 1.0); MAX_TRACKS + 1],
    ] {
        assert_eq!(
            program(tracks).validate(base()).unwrap_err().code,
            Code::Budget
        );
    }
    for count in [0, 1, MAX_KEYS_PER_TRACK + 1] {
        let mut item = track(1, Property::Opacity, 0.0, 1.0);
        item.keys = vec![
            Key {
                time_ms: 0,
                value: 1.0
            };
            count
        ];
        assert_eq!(
            program(vec![item]).validate(base()).unwrap_err().code,
            Code::Budget
        );
    }
    let layers = (1..=33).map(|id| group(id, 0, 1.0)).collect();
    let tracks = (1..=33)
        .map(|id| Track {
            target: id,
            property: Property::TranslateX,
            keys: (0..64)
                .map(|i| Key {
                    time_ms: if i == 63 { 1000 } else { i * 15 },
                    value: 0.0,
                })
                .collect(),
        })
        .collect();
    assert_eq!(
        program(tracks)
            .validate(scene_with(layers, 640, 480))
            .unwrap_err()
            .code,
        Code::Budget
    );
}
#[test]
fn envelopes_check_combined_ancestor_scale_and_negative_coordinates() {
    let base = scene_with(vec![group(1, 0, 8.0), group(2, 1, 1.0)], 640, 480);
    assert_eq!(
        program(vec![track(2, Property::ScaleX, 1.0, 16.0)])
            .validate(base)
            .unwrap_err()
            .code,
        Code::Envelope
    );
    let mut parent = group(1, 0, 16.0);
    parent.bounds.width = 1.0;
    parent.bounds.height = 1.0;
    let mut child = group(2, 1, 4.0);
    child.bounds.width = 1.0;
    child.bounds.height = 1.0;
    let base = scene_with(vec![parent, child, group(3, 2, 1.0)], 640, 480);
    assert_eq!(
        program(vec![track(3, Property::TranslateX, -32768.0, 0.0)])
            .validate(base)
            .unwrap_err()
            .code,
        Code::Envelope
    );
}
#[test]
fn opacity_reservation_counts_all_animated_targets_even_at_one() {
    for (count, width, height) in [(9, 640, 480), (6, 3200, 2000)] {
        let layers = (1..=count).map(|id| group(id, 0, 1.0)).collect();
        let tracks = (1..=count)
            .map(|id| track(id, Property::Opacity, 1.0, 1.0))
            .collect();
        assert_eq!(
            program(tracks)
                .validate(scene_with(layers, width, height))
                .unwrap_err()
                .code,
            Code::Budget
        );
    }
}
#[test]
fn clock_freezes_across_multiple_idempotent_pauses_without_catchup() {
    let start = Instant::now();
    let mut clock = Clock::new(start);
    assert_eq!(
        clock.advance(start + Duration::from_millis(250)).unwrap(),
        Duration::from_millis(250)
    );
    clock
        .set_paused(start + Duration::from_millis(500), true)
        .unwrap();
    clock
        .set_paused(start + Duration::from_secs(5), true)
        .unwrap();
    assert!(clock.paused());
    assert_eq!(
        clock.advance(start + Duration::from_secs(10)).unwrap(),
        Duration::from_millis(500)
    );
    clock
        .set_paused(start + Duration::from_secs(20), false)
        .unwrap();
    clock
        .set_paused(start + Duration::from_secs(20), false)
        .unwrap();
    assert_eq!(
        clock
            .advance(start + Duration::from_millis(20_500))
            .unwrap(),
        Duration::from_secs(1)
    );
    clock
        .set_paused(start + Duration::from_secs(21), true)
        .unwrap();
    clock
        .set_paused(start + Duration::from_secs(31), false)
        .unwrap();
    assert_eq!(
        clock
            .advance(start + Duration::from_millis(31_500))
            .unwrap(),
        Duration::from_secs(2)
    );
}
#[test]
fn backwards_clock_input_is_rejected_atomically_including_pause_changes() {
    let start = Instant::now();
    let mut clock = Clock::new(start);
    clock.advance(start + Duration::from_secs(1)).unwrap();
    assert_eq!(
        clock.set_paused(start, true).unwrap_err().code,
        Code::Timing
    );
    assert!(!clock.paused());
    assert_eq!(clock.elapsed(), Duration::from_secs(1));
    assert_eq!(
        clock.advance(start + Duration::from_secs(2)).unwrap(),
        Duration::from_secs(2)
    );
}
#[test]
fn paused_sampling_retains_pose_and_resume_advances_only_running_time() {
    let animation = program(vec![track(1, Property::TranslateX, 0.0, 100.0)])
        .validate(base())
        .unwrap();
    let start = Instant::now();
    let mut clock = Clock::new(start);
    clock
        .set_paused(start + Duration::from_millis(400), true)
        .unwrap();
    let frozen = animation
        .sample(clock.advance(start + Duration::from_secs(50)).unwrap())
        .unwrap();
    assert_eq!(value(&frozen, Property::TranslateX), 40.0);
    clock
        .set_paused(start + Duration::from_secs(60), false)
        .unwrap();
    let resumed = animation
        .sample(
            clock
                .advance(start + Duration::from_millis(60_100))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(value(&resumed, Property::TranslateX), 50.0);
    assert!(resumed.active);
}

#[test]
fn maximum_total_key_budget_and_zero_scale_roundtrip_remain_valid() {
    let layers = (1..=MAX_TRACKS as u32)
        .map(|id| group(id, 0, 1.0))
        .collect();
    let tracks = (1..=MAX_TRACKS as u32)
        .map(|id| Track {
            target: id,
            property: Property::TranslateX,
            keys: (0..16)
                .map(|i| Key {
                    time_ms: if i == 15 { 1000 } else { i * 66 },
                    value: i as f32,
                })
                .collect(),
        })
        .collect();
    let animation = program(tracks)
        .validate(scene_with(layers, 640, 480))
        .unwrap();
    assert_eq!(
        animation
            .animation()
            .tracks
            .iter()
            .map(|t| t.keys.len())
            .sum::<usize>(),
        MAX_KEYS
    );
    let sample = animation.sample(Duration::from_millis(500)).unwrap();
    assert_eq!(
        hierarchy::decode(&sample.scene.encode()).unwrap(),
        sample.scene
    );
    let animation = program(vec![track(1, Property::ScaleX, 0.0, 16.0)])
        .validate(base())
        .unwrap();
    let sample = animation.sample(Duration::ZERO).unwrap();
    assert_eq!(sample.scene.resolved()[1].bounds.width, 0.0);
    assert_eq!(
        hierarchy::decode(&sample.scene.encode()).unwrap(),
        sample.scene
    );
}

#[test]
fn elapsed_overflow_fails_without_changing_clock_state() {
    let start = Instant::now();
    let mut clock = Clock {
        last: start,
        elapsed: Duration::MAX,
        paused: false,
    };
    assert_eq!(
        clock
            .set_paused(start + Duration::from_nanos(1), true)
            .unwrap_err()
            .code,
        Code::Timing
    );
    assert_eq!(clock.elapsed(), Duration::MAX);
    assert!(!clock.paused());
    assert_eq!(clock.last, start);
}

#[test]
fn valid_endpoint_frames_do_not_admit_unsafe_intermediate_ancestor_products() {
    // At both endpoints the world scale is 16, but halfway it is 8.5*8.5=72.25.
    let start = scene_with(vec![group(1, 0, 1.0), group(2, 1, 16.0)], 640, 480);
    let end = scene_with(vec![group(1, 0, 16.0), group(2, 1, 1.0)], 640, 480);
    assert_eq!(start.resolved()[1].transform.scale[0], 16.0);
    assert_eq!(end.resolved()[1].transform.scale[0], 16.0);
    let animation = program(vec![
        track(1, Property::ScaleX, 1.0, 16.0),
        track(2, Property::ScaleX, 16.0, 1.0),
    ]);
    assert_eq!(animation.validate(start).unwrap_err().code, Code::Envelope);
}
