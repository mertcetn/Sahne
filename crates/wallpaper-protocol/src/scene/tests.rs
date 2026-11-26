use super::*;
use crate::{read_frame, write_frame};
use std::collections::BTreeSet;
use wallpaper_scene::{Content, Node, Rect, SCENE_VERSION, Scene};

fn message(body: Body, sequence: u64, base_revision: u64) -> Message {
    Message {
        session: 42,
        sequence,
        base_revision,
        body,
    }
}

fn text(value: &str) -> Edit {
    Edit::Text {
        node: 1,
        value: value.into(),
    }
}

fn encode(message: &Message) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_frame(&mut bytes, message).unwrap();
    bytes
}

#[test]
fn interval_registration_enforces_scheduler_delay_limits() {
    for delay_ms in [16, 32, 86_400_000] {
        let request = message(Body::RegisterInterval { delay_ms }, 3, 0);
        assert_eq!(
            read_frame(&mut encode(&request).as_slice()).unwrap(),
            Some(request)
        );
    }
    for delay_ms in [0u32, 15, 86_400_001, u32::MAX] {
        let invalid = message(Body::RegisterInterval { delay_ms }, 3, 0);
        let mut output = Vec::new();
        assert!(write_frame(&mut output, &invalid).is_err());
        assert!(output.is_empty());
        let mut frame = encode(&message(Body::RegisterInterval { delay_ms: 32 }, 3, 0));
        frame[31..].copy_from_slice(&delay_ms.to_le_bytes());
        assert!(read_frame(&mut frame.as_slice()).is_err());
    }
}

#[test]
fn interval_cancellation_roundtrips_without_payload() {
    for (body, tag) in [(Body::CancelInterval, 15), (Body::IntervalCancelled, 16)] {
        let request = message(body, 4, 3);
        let mut frame = encode(&request);
        assert_eq!(&frame[30..], &[tag]);
        let decoded = read_frame(&mut frame.as_slice()).unwrap().unwrap();
        assert_eq!(decoded, request);
        assert!(decoded.check_context(42, 4, 3).is_ok());
        assert!(decoded.check_context(42, 4, 2).is_err());
        frame[0] += 1;
        frame.push(0);
        assert!(read_frame(&mut frame.as_slice()).is_err());
    }
}

#[test]
fn scene_program_checks_source_bounds_and_encoding() {
    let program = |source: String| {
        message(
            Body::SceneProgram {
                targets: BTreeMap::from([("clock".into(), 3)]),
                source,
                initial: Vec::new(),
            },
            2,
            0,
        )
    };
    let valid = program("x".repeat(MAX_SCRIPT_SOURCE_BYTES));
    let wire = encode(&valid);
    assert_eq!(read_frame(&mut wire.as_slice()).unwrap(), Some(valid));
    for source in [
        String::new(),
        "x".repeat(MAX_SCRIPT_SOURCE_BYTES + 1),
        "a\0b".into(),
    ] {
        let mut output = Vec::new();
        assert!(write_frame(&mut output, &program(source)).is_err());
        assert!(output.is_empty());
    }
    for byte in [0, 0xff] {
        let mut wire = encode(&program("x".into()));
        *wire.last_mut().unwrap() = byte;
        assert!(read_frame(&mut wire.as_slice()).is_err());
    }
}

#[test]
fn scene_program_initial_properties_are_bound_and_unique() {
    let program = |initial| {
        message(
            Body::SceneProgram {
                targets: BTreeMap::from([("clock".into(), 1)]),
                source: "() => {}".into(),
                initial,
            },
            2,
            0,
        )
    };
    let valid = program(vec![
        text("old"),
        Edit::Color {
            node: 1,
            value: Color([255; 4]),
        },
    ]);
    let wire = encode(&valid);
    assert_eq!(read_frame(&mut wire.as_slice()).unwrap(), Some(valid));
    let mut unbound = wire;
    let last_color_node = unbound.len() - 8;
    unbound[last_color_node] = 2;
    assert!(read_frame(&mut unbound.as_slice()).is_err());
    let mut output = Vec::new();
    assert!(write_frame(&mut output, &program(vec![text("old"), text("new")])).is_err());
    assert!(output.is_empty());
}

#[test]
fn scene_binding_table_roundtrips_and_rejects_invalid_entries() {
    let targets = BTreeMap::from([("a".into(), 3), ("b".into(), 5)]);
    let binding = message(
        Body::SceneBindings {
            targets: targets.clone(),
        },
        2,
        0,
    );
    let encoded = encode(&binding);
    assert_eq!(read_frame(&mut encoded.as_slice()).unwrap(), Some(binding));

    for (name, node, revision) in [
        ("", 5, 0),
        ("b", 0, 0),
        ("b", 3, 0),
        ("b", 5, 1),
        ("1b", 5, 0),
    ] {
        let invalid = message(
            Body::SceneBindings {
                targets: BTreeMap::from([("a".into(), 3), (name.into(), node)]),
            },
            2,
            revision,
        );
        let mut output = Vec::new();
        assert!(write_frame(&mut output, &invalid).is_err());
        assert!(output.is_empty());
    }
    for (offset, value) in [
        (22, 1),
        (31, 0),
        (31, 65),
        (32, 65),
        (33, 0xff),
        (39, b'a'),
        (40, 3),
        (40, 0),
    ] {
        let mut invalid = encoded.clone();
        invalid[offset] = value;
        assert!(read_frame(&mut invalid.as_slice()).is_err());
    }
}
#[test]
fn scene_batch_roundtrips_and_bad_wire_is_rejected() {
    let batch = message(
        Body::SceneBatch {
            edits: vec![
                text("é"),
                Edit::Color {
                    node: 1,
                    value: Color([1, 2, 3, 4]),
                },
            ],
        },
        1,
        0,
    );
    let bytes = encode(&batch);
    assert_eq!(&bytes[30..40], &[6, 2, 0, 1, 1, 0, 0, 0, 2, 0]);
    assert_eq!(read_frame(&mut bytes.as_slice()).unwrap(), Some(batch));
    for length in 1..bytes.len() {
        assert!(read_frame(&mut &bytes[..length]).is_err());
    }
    for (offset, value) in [(31, 65), (33, 0xff), (40, 0xff)] {
        let mut malformed = bytes.clone();
        malformed[offset] = value;
        assert!(read_frame(&mut malformed.as_slice()).is_err());
    }
}

#[test]
fn scene_encoding_rejects_oversized_batches_before_writing() {
    let full = "x".repeat(MAX_TEXT_BYTES);
    let mut edits = vec![text(&full); 3];
    let mut bytes = Vec::new();
    write_frame(
        &mut bytes,
        &message(
            Body::SceneBatch {
                edits: edits.clone(),
            },
            1,
            0,
        ),
    )
    .unwrap();
    edits.push(text("x"));
    let mut rejected = Vec::new();
    assert!(write_frame(&mut rejected, &message(Body::SceneBatch { edits }, 1, 0)).is_err());
    assert!(rejected.is_empty());
    assert!(read_frame(&mut bytes.as_slice()).is_ok());
}

#[test]
fn ack_requires_exact_checked_revision_on_read_and_write() {
    let ack = message(Body::SceneAck { revision: 2 }, 2, 1);
    let mut bytes = encode(&ack);
    assert_eq!(read_frame(&mut bytes.as_slice()).unwrap(), Some(ack));
    bytes[31] = 3;
    assert!(read_frame(&mut bytes.as_slice()).is_err());
    let mut output = Vec::new();
    assert!(
        write_frame(
            &mut output,
            &message(Body::SceneAck { revision: 0 }, 1, u64::MAX)
        )
        .is_err()
    );
    assert!(output.is_empty());
}

#[test]
fn clock_tick_carries_only_a_valid_utc_minute() {
    for minute in [0, 1439] {
        let tick = message(Body::ClockTick { utc_minute: minute }, 2, 0);
        let bytes = encode(&tick);
        assert_eq!(bytes[30], 8);
        assert_eq!(read_frame(&mut bytes.as_slice()).unwrap(), Some(tick));
    }
    let invalid = message(Body::ClockTick { utc_minute: 1440 }, 2, 0);
    let mut output = Vec::new();
    assert!(write_frame(&mut output, &invalid).is_err());
    assert!(output.is_empty());
    let mut bytes = encode(&message(Body::ClockTick { utc_minute: 1439 }, 2, 0));
    bytes[31..33].copy_from_slice(&1440u16.to_le_bytes());
    assert!(read_frame(&mut bytes.as_slice()).is_err());
}

#[test]
fn decoded_batches_produce_ack_then_reject_without_losing_accepted_scene() {
    let scene = Scene {
        version: SCENE_VERSION,
        width: 100,
        height: 100,
        background: Color([0; 4]),
        resources: vec![],
        nodes: vec![Node {
            id: 1,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            content: Content::Text {
                text: "old".into(),
                font_size: 16.0,
                color: Color([255; 4]),
            },
        }],
    }
    .validate()
    .unwrap();
    let mut host = HostScene::new(scene, 42, &BTreeSet::from([1])).unwrap();
    let request = message(
        Body::SceneBatch {
            edits: vec![text("new")],
        },
        1,
        0,
    );
    let decoded = read_frame(&mut encode(&request).as_slice())
        .unwrap()
        .unwrap();
    let ack = accept_scene_batch(&mut host, decoded);
    assert_eq!(
        read_frame(&mut encode(&ack).as_slice()).unwrap(),
        Some(message(Body::SceneAck { revision: 1 }, 1, 0))
    );
    let accepted = host.scene().encode();
    let reply = accept_scene_batch(&mut host, request);
    assert!(matches!(
        reply.body,
        Body::Reject {
            code: SCENE_REJECT_CODE,
            ..
        }
    ));
    assert!(host.is_closed());
    assert_eq!(host.revision(), 1);
    assert_eq!(host.scene().encode(), accepted);
}
