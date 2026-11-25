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
