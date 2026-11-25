use super::*;
use crate::{Node, Rect, SCENE_VERSION, Scene};

fn host() -> HostScene {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 40.0,
    };
    let scene = Scene {
        version: SCENE_VERSION,
        width: 1920,
        height: 1080,
        background: Color([0, 0, 0, 255]),
        resources: vec![],
        nodes: vec![
            Node {
                id: 10,
                bounds,
                content: Content::Text {
                    text: "original".into(),
                    font_size: 20.0,
                    color: Color([255; 4]),
                },
            },
            Node {
                id: 20,
                bounds,
                content: Content::Rectangle {
                    color: Color([255; 4]),
                },
            },
            Node {
                id: 30,
                bounds,
                content: Content::Rectangle {
                    color: Color([255; 4]),
                },
            },
        ],
    }
    .validate()
    .unwrap();
    HostScene::new(scene, 7, &BTreeSet::from([10, 20])).unwrap()
}

fn batch(sequence: u64, base_revision: u64, edits: Vec<Edit>) -> Batch {
    Batch {
        session: 7,
        sequence,
        base_revision,
        edits,
    }
}

fn text(value: &str) -> Edit {
    Edit::Text {
        node: 10,
        value: value.into(),
    }
}

#[test]
fn successful_batch_acknowledges_exactly_one_validated_revision() {
    let mut host = host();
    let color = Color([10, 20, 30, 255]);
    let ack = host
        .apply(batch(
            1,
            0,
            vec![
                text("12:34"),
                Edit::Color {
                    node: 20,
                    value: color,
                },
            ],
        ))
        .unwrap();
    assert_eq!(
        ack,
        Ack {
            session: 7,
            sequence: 1,
            revision: 1
        }
    );
    let nodes = &host.scene().scene().nodes;
    assert!(matches!(&nodes[0].content, Content::Text { text, .. } if text == "12:34"));
    assert_eq!(nodes[1].content, Content::Rectangle { color });
    assert_eq!(
        nodes[2].content,
        Content::Rectangle {
            color: Color([255; 4])
        }
    );
    assert_eq!(
        crate::decode(&host.scene().encode()).unwrap(),
        *host.scene()
    );
    assert_eq!(
        host.apply(batch(2, 1, vec![text("12:35")]))
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn final_scene_validation_failure_rolls_back_earlier_edit_and_closes() {
    let mut host = host();
    host.apply(batch(1, 0, vec![text("accepted")])).unwrap();
    let accepted = host.scene().encode();
    assert_eq!(
        host.apply(batch(
            2,
            1,
            vec![
                Edit::Color {
                    node: 20,
                    value: Color([0; 4])
                },
                text("invalid\0text"),
            ]
        )),
        Err(Reject::Scene)
    );
    assert_eq!(host.scene().encode(), accepted);
    assert_eq!(host.revision(), 1);
    assert_eq!(
        host.apply(batch(2, 1, vec![text("valid")])),
        Err(Reject::Closed)
    );
}

#[test]
fn bad_envelopes_and_replays_preserve_last_accepted_scene() {
    for (session, sequence, revision) in [(8, 2, 1), (7, 1, 0), (7, 3, 1), (7, 2, 0)] {
        let mut host = host();
        host.apply(batch(1, 0, vec![text("accepted")])).unwrap();
        let accepted = host.scene().encode();
        let mut invalid = batch(sequence, revision, vec![text("rejected")]);
        invalid.session = session;
        assert_eq!(host.apply(invalid), Err(Reject::Context));
        assert!(host.is_closed());
        assert_eq!(host.scene().encode(), accepted);
        assert_eq!(host.revision(), 1);
    }
}

#[test]
fn budget_target_and_property_checks_run_before_commit() {
    let color = Color([0; 4]);
    for (edits, rejection) in [
        (vec![], Reject::Budget),
        ((0..=MAX_EDITS).map(|_| text("")).collect(), Reject::Budget),
        (vec![text(&"x".repeat(MAX_TEXT_BYTES + 1))], Reject::Budget),
        (vec![text("one"), text("two")], Reject::Duplicate),
        (
            vec![Edit::Color {
                node: 30,
                value: color,
            }],
            Reject::Target,
        ),
        (
            vec![Edit::Text {
                node: 20,
                value: "bad".into(),
            }],
            Reject::Property,
        ),
    ] {
        let mut host = host();
        let original = host.scene().encode();
        assert_eq!(host.apply(batch(1, 0, edits)), Err(rejection));
        assert!(host.is_closed());
        assert_eq!(host.scene().encode(), original);
        assert_eq!(host.revision(), 0);
    }
}

#[test]
fn transport_close_keeps_committed_scene_even_if_ack_was_not_delivered() {
    let mut host = host();
    host.apply(batch(1, 0, vec![text("accepted")])).unwrap();
    let accepted = host.scene().encode();
    host.close();
    assert_eq!(
        host.apply(batch(2, 1, vec![text("late")])),
        Err(Reject::Closed)
    );
    assert_eq!(host.scene().encode(), accepted);
    assert_eq!(host.revision(), 1);
}
