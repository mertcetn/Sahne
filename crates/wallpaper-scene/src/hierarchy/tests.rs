use super::*;

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn group(id: u32, parent: u32) -> Layer {
    Layer {
        id,
        parent,
        bounds: rect(0.0, 0.0, 10.0, 10.0),
        content: LayerContent::Group {
            transform: Transform::default(),
            opacity: 1.0,
            clip: false,
        },
    }
}
fn draw(id: u32, parent: u32) -> Layer {
    Layer {
        id,
        parent,
        bounds: rect(0.0, 0.0, 10.0, 10.0),
        content: LayerContent::Draw(Content::Rectangle {
            color: Color([255; 4]),
        }),
    }
}
fn with_layers(layers: Vec<Layer>) -> Hierarchy {
    Hierarchy {
        version: VERSION,
        width: 1920,
        height: 1080,
        background: Color([0; 4]),
        resources: vec![],
        layers,
    }
}
fn effects(layer: &mut Layer) -> (&mut Transform, &mut f32, &mut bool) {
    let LayerContent::Group {
        transform,
        opacity,
        clip,
    } = &mut layer.content
    else {
        panic!("Expected group")
    };
    (transform, opacity, clip)
}
fn code(input: Hierarchy) -> ErrorCode {
    input.validate().unwrap_err().code
}

#[test]
fn groups_preserve_order_alpha_boundaries_and_roundtrip() {
    let validated = fixtures::original().validate().unwrap();
    assert_eq!(
        validated.commands(),
        [
            Command::BeginGroup(0),
            Command::Draw(1),
            Command::Draw(2),
            Command::EndGroup(0),
            Command::Draw(3)
        ]
    );
    assert_eq!(
        validated.resolved()[0].bounds,
        rect(90.0, 100.0, 640.0, 480.0)
    );
    assert_eq!(
        validated.resolved()[2].bounds,
        rect(190.0, 200.0, 200.0, 200.0)
    );
    assert_eq!(
        validated.resolved()[2].clip,
        rect(90.0, 100.0, 640.0, 480.0)
    );
    assert_eq!(validated.resolved()[3].transform.scale, [1.0, 1.0]);
    assert!(matches!(
        validated.scene().layers[0].content,
        LayerContent::Group { opacity: 0.5, .. }
    ));
    assert!(matches!(
        validated.scene().layers[1].content,
        LayerContent::Draw(Content::Rectangle {
            color: Color([255, 0, 0, 255])
        })
    ));
    let bytes = validated.encode();
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded, validated);
    assert_eq!(decoded.encode(), bytes);
    assert_eq!(
        crate::decode(&bytes).unwrap_err().code,
        ErrorCode::UnsupportedVersion
    );
    let legacy = Scene {
        version: SCENE_VERSION,
        width: 1,
        height: 1,
        background: Color([0; 4]),
        resources: vec![],
        nodes: vec![],
    }
    .validate()
    .unwrap()
    .encode();
    assert_eq!(
        decode(&legacy).unwrap_err().code,
        ErrorCode::UnsupportedVersion
    );
}

#[test]
fn nested_transform_order_and_clip_intersection_are_explicit() {
    let mut outer = group(1, 0);
    outer.bounds = rect(100.0, 50.0, 100.0, 100.0);
    effects(&mut outer).0.translate = [10.0, 20.0];
    effects(&mut outer).0.scale = [2.0, 3.0];
    *effects(&mut outer).2 = true;
    let mut inner = group(2, 1);
    inner.bounds = rect(20.0, 20.0, 100.0, 100.0);
    effects(&mut inner).0.translate = [5.0, 10.0];
    effects(&mut inner).0.scale = [0.5, 2.0];
    *effects(&mut inner).2 = true;
    let mut leaf = draw(3, 2);
    leaf.bounds = rect(-50.0, -50.0, 300.0, 300.0);
    let validated = with_layers(vec![outer, inner, leaf]).validate().unwrap();
    assert_eq!(
        validated.resolved()[1].transform,
        Transform {
            translate: [160.0, 160.0],
            scale: [1.0, 6.0]
        }
    );
    assert_eq!(
        validated.resolved()[1].clip,
        rect(160.0, 160.0, 100.0, 210.0)
    );
    assert_eq!(
        validated.resolved()[2].bounds,
        rect(110.0, -140.0, 300.0, 1800.0)
    );
    assert_eq!(validated.resolved()[2].clip, validated.resolved()[1].clip);
    assert_eq!(
        validated.commands(),
        [
            Command::BeginGroup(0),
            Command::BeginGroup(1),
            Command::Draw(2),
            Command::EndGroup(1),
            Command::EndGroup(0)
        ]
    );
}

#[test]
fn parent_must_be_an_open_group_not_forward_leaf_or_reopened_subtree() {
    for layers in [
        vec![draw(1, 1)],
        vec![draw(1, 2), group(2, 0)],
        vec![draw(1, 0), draw(2, 1)],
        vec![group(1, 0), draw(2, 0), draw(3, 1)],
        vec![group(1, 0), group(2, 1), draw(3, 1), draw(4, 2)],
    ] {
        assert_eq!(code(with_layers(layers)), ErrorCode::InvalidHierarchy);
    }
    assert_eq!(
        code(with_layers(vec![group(1, 0), draw(1, 1)])),
        ErrorCode::DuplicateNode
    );
    assert_eq!(code(with_layers(vec![group(0, 0)])), ErrorCode::InvalidNode);
}

#[test]
fn depth_and_node_limits_include_groups_without_recursion() {
    let mut layers: Vec<_> = (1..=MAX_DEPTH as u32).map(|id| group(id, id - 1)).collect();
    layers.push(draw(100, MAX_DEPTH as u32));
    with_layers(layers.clone()).validate().unwrap();
    layers.pop();
    layers.push(group(100, MAX_DEPTH as u32));
    assert_eq!(code(with_layers(layers)), ErrorCode::BudgetExceeded);
    with_layers((1..=MAX_NODES as u32).map(|id| group(id, 0)).collect())
        .validate()
        .unwrap();
    assert_eq!(
        code(with_layers(
            (1..=MAX_NODES as u32 + 1).map(|id| group(id, 0)).collect()
        )),
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn all_effect_numbers_are_finite_and_local_ranges_are_enforced() {
    for field in 0..5 {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut layer = group(1, 0);
            let (transform, opacity, _) = effects(&mut layer);
            match field {
                0 | 1 => transform.translate[field] = value,
                2 | 3 => transform.scale[field - 2] = value,
                _ => *opacity = value,
            }
            assert_eq!(code(with_layers(vec![layer])), ErrorCode::NonFinite);
        }
    }
    for (scale, opacity) in [
        ([-0.1, 1.0], 1.0),
        ([16.1, 1.0], 1.0),
        ([1.0, 1.0], -0.01),
        ([1.0, 1.0], 1.01),
    ] {
        let mut layer = group(1, 0);
        effects(&mut layer).0.scale = scale;
        *effects(&mut layer).1 = opacity;
        assert_eq!(code(with_layers(vec![layer])), ErrorCode::InvalidEffect);
    }
    let mut layer = group(1, 0);
    effects(&mut layer).0.translate = [32768.1, 0.0];
    assert_eq!(code(with_layers(vec![layer])), ErrorCode::InvalidEffect);
}

#[test]
fn accumulated_transform_limits_and_zero_scale_are_checked() {
    let mut outer = group(1, 0);
    effects(&mut outer).0.scale = [16.0, 16.0];
    let mut inner = group(2, 1);
    effects(&mut inner).0.scale = [4.0, 4.0];
    with_layers(vec![outer.clone(), inner.clone()])
        .validate()
        .unwrap();
    effects(&mut inner).0.scale = [4.01, 4.0];
    assert_eq!(
        code(with_layers(vec![outer.clone(), inner])),
        ErrorCode::InvalidEffect
    );
    let mut leaf = draw(2, 1);
    leaf.bounds = rect(32768.0, 0.0, 32768.0, 0.0);
    with_layers(vec![outer.clone(), leaf.clone()])
        .validate()
        .unwrap();
    outer.bounds.x = 1.0;
    assert_eq!(
        code(with_layers(vec![outer, leaf])),
        ErrorCode::InvalidEffect
    );
    let mut zero = group(1, 0);
    effects(&mut zero).0.scale = [0.0, 0.0];
    *effects(&mut zero).2 = true;
    let validated = with_layers(vec![zero, draw(2, 1)]).validate().unwrap();
    assert_eq!(validated.resolved()[1].bounds, rect(0.0, 0.0, 0.0, 0.0));
    assert_eq!(validated.resolved()[1].clip, rect(0.0, 0.0, 0.0, 0.0));
}

#[test]
fn offscreen_budget_counts_zero_opacity_and_sequential_groups() {
    let layers = || {
        (1..=MAX_OPACITY_GROUPS as u32)
            .map(|id| {
                let mut g = group(id, 0);
                *effects(&mut g).1 = 0.0;
                g
            })
            .collect::<Vec<_>>()
    };
    with_layers(layers()).validate().unwrap();
    let mut many = layers();
    let mut extra = group(100, 0);
    *effects(&mut extra).1 = 0.5;
    many.push(extra);
    assert_eq!(code(with_layers(many)), ErrorCode::BudgetExceeded);
    let mut big = with_layers(vec![group(1, 0), group(2, 0)]);
    big.width = 8192;
    big.height = 4096;
    *effects(&mut big.layers[0]).1 = 0.5;
    big.clone().validate().unwrap();
    *effects(&mut big.layers[1]).1 = 0.5;
    assert_eq!(code(big), ErrorCode::BudgetExceeded);
}

#[test]
fn v2_wire_truncation_flags_versions_and_lengths_fail_closed() {
    let bytes = fixtures::original().validate().unwrap().encode();
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err(), "{end}");
    }
    let mut changed = bytes.clone();
    changed.push(0);
    assert_eq!(
        decode(&changed).unwrap_err().code,
        ErrorCode::InvalidEncoding
    );
    for (offset, value, expected) in [
        (8, 99, ErrorCode::UnsupportedVersion),
        (54, 99, ErrorCode::InvalidEncoding),
        (75, 2, ErrorCode::InvalidEncoding),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        assert_eq!(decode(&changed).unwrap_err().code, expected);
    }
    for offset in [22, 26] {
        let mut changed = bytes.clone();
        changed[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            decode(&changed).unwrap_err().code,
            ErrorCode::BudgetExceeded
        );
    }
    let mut changed = bytes[..30].to_vec();
    changed[26..30].copy_from_slice(&4096u32.to_le_bytes());
    assert_eq!(decode(&changed).unwrap_err().code, ErrorCode::Truncated);
    let mut text = draw(1, 0);
    text.content = LayerContent::Draw(Content::Text {
        text: "x".into(),
        font_size: 16.0,
        color: Color([255; 4]),
    });
    let mut bytes = with_layers(vec![text]).validate().unwrap().encode();
    bytes[63..67].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode(&bytes).unwrap_err().code, ErrorCode::BudgetExceeded);
    assert_eq!(
        decode(&vec![0; MAX_SCENE_BYTES + 1]).unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn resource_text_and_group_geometry_reuse_the_existing_bounds() {
    let mut scene = with_layers(vec![draw(1, 0)]);
    scene.layers[0].content = LayerContent::Draw(Content::Image { resource: 7 });
    assert_eq!(code(scene.clone()), ErrorCode::UnknownResource);
    scene.resources.push(ImageResource {
        id: 7,
        width: 2,
        height: 2,
    });
    scene.clone().validate().unwrap();
    scene.resources.push(ImageResource {
        id: 7,
        width: 2,
        height: 2,
    });
    assert_eq!(code(scene), ErrorCode::DuplicateResource);
    let mut bad = group(1, 0);
    bad.bounds.width = f32::NAN;
    assert_eq!(code(with_layers(vec![bad])), ErrorCode::NonFinite);
    let mut bad = group(1, 0);
    bad.bounds.width = -1.0;
    assert_eq!(code(with_layers(vec![bad])), ErrorCode::InvalidGeometry);
    for text in ["\0".into(), "x".repeat(MAX_TEXT_BYTES + 1)] {
        let expected = if text.len() > MAX_TEXT_BYTES {
            ErrorCode::BudgetExceeded
        } else {
            ErrorCode::InvalidText
        };
        let mut bad = draw(1, 0);
        bad.content = LayerContent::Draw(Content::Text {
            text,
            font_size: 24.0,
            color: Color([255; 4]),
        });
        assert_eq!(code(with_layers(vec![bad])), expected);
    }
}

#[test]
fn deterministic_mutations_revalidate_derived_order_and_geometry() {
    let bytes = fixtures::original().validate().unwrap().encode();
    for seed in 0..1024usize {
        let mut changed = bytes.clone();
        changed[(seed * 31 + 7) % bytes.len()] ^= 1 << (seed % 8);
        if let Ok(scene) = decode(&changed) {
            assert_eq!(decode(&scene.encode()).unwrap(), scene);
            assert!(scene.commands().len() <= 2 * MAX_NODES);
        }
    }
}

#[test]
fn minimal_v2_bytes_and_combined_maximum_envelope_are_bounded() {
    let empty = with_layers(vec![]).validate().unwrap().encode();
    assert_eq!(empty.len(), 30);
    assert_eq!(&empty[..10], b"SAHNSCN\0\x02\x00");
    let one = with_layers(vec![draw(1, 0)]).validate().unwrap().encode();
    assert_eq!(one.len(), 59);
    assert_eq!(&one[30..38], &[1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(one[54], 1);
    assert_eq!(&one[55..], [255; 4]);
    let mut layers: Vec<_> = (1..=MAX_NODES as u32).map(|id| group(id, 0)).collect();
    for layer in layers
        .iter_mut()
        .take(MAX_TOTAL_TEXT_BYTES / MAX_TEXT_BYTES)
    {
        layer.content = LayerContent::Draw(Content::Text {
            text: "x".repeat(MAX_TEXT_BYTES),
            font_size: 24.0,
            color: Color([255; 4]),
        });
    }
    let mut scene = with_layers(layers);
    scene.resources = (1..=MAX_RESOURCES as u32)
        .map(|id| ImageResource {
            id,
            width: 1,
            height: 1,
        })
        .collect();
    let validated = scene.clone().validate().unwrap();
    let bytes = validated.encode();
    assert!(bytes.len() <= MAX_SCENE_BYTES);
    assert_eq!(decode(&bytes).unwrap(), validated);
    scene.layers[MAX_TOTAL_TEXT_BYTES / MAX_TEXT_BYTES].content =
        LayerContent::Draw(Content::Text {
            text: "x".into(),
            font_size: 24.0,
            color: Color([255; 4]),
        });
    assert_eq!(code(scene), ErrorCode::BudgetExceeded);
}
