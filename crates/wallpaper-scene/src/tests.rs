use super::*;

fn empty() -> Scene {
    Scene {
        version: SCENE_VERSION,
        width: 1920,
        height: 1080,
        background: Color([8, 16, 24, 255]),
        resources: vec![],
        nodes: vec![],
    }
}
fn node(id: u32, content: Content) -> Node {
    Node {
        id,
        bounds: Rect {
            x: 10.0,
            y: 20.0,
            width: 320.0,
            height: 120.0,
        },
        content,
    }
}
fn text(value: String) -> Content {
    Content::Text {
        text: value,
        font_size: 24.0,
        color: Color([255; 4]),
    }
}
fn fixture() -> Scene {
    let mut scene = empty();
    scene.resources.push(ImageResource {
        id: 7,
        width: 2,
        height: 2,
    });
    scene.nodes = vec![
        node(
            1,
            Content::Rectangle {
                color: Color([40, 60, 80, 128]),
            },
        ),
        node(2, text("Sahne — Türkçe\n日本語".into())),
        node(3, Content::Image { resource: 7 }),
    ];
    scene
}

#[test]
fn deterministic_roundtrip_preserves_draw_order_unicode_and_ids() {
    let scene = fixture().validate().unwrap();
    let bytes = scene.encode();
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded, scene);
    assert_eq!(decoded.encode(), bytes);
    assert_eq!(
        decoded
            .scene()
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[test]
fn minimal_scene_has_exact_versioned_wire_bytes() {
    let mut scene = empty();
    scene.width = 1;
    scene.height = 1;
    scene.background = Color([1, 2, 3, 4]);
    let bytes = scene.validate().unwrap().encode();
    assert_eq!(
        bytes,
        [
            83, 65, 72, 78, 83, 67, 78, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0, 0,
            0, 0, 0
        ]
    );
}

#[test]
fn every_truncation_and_trailing_data_is_rejected() {
    let bytes = fixture().validate().unwrap().encode();
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err(), "cut={end}");
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode(&trailing).unwrap_err().code,
        ErrorCode::InvalidEncoding
    );
}

#[test]
fn rejects_unknown_versions_tags_and_magic() {
    let mut scene = empty();
    scene.nodes.push(node(1, text("ok".into())));
    let bytes = scene.validate().unwrap().encode();
    for (offset, value, expected) in [
        (0, 0, ErrorCode::InvalidEncoding),
        (8, 2, ErrorCode::UnsupportedVersion),
        (50, 255, ErrorCode::InvalidEncoding),
    ] {
        let mut mutated = bytes.clone();
        mutated[offset] = value;
        assert_eq!(decode(&mutated).unwrap_err().code, expected);
    }
}

#[test]
fn hostile_counts_lengths_and_input_size_fail_before_payload_allocation() {
    let mut scene = empty();
    scene.nodes.push(node(1, text("ok".into())));
    let bytes = scene.validate().unwrap().encode();
    for offset in [22, 26, 59] {
        let mut mutated = bytes.clone();
        mutated[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            decode(&mutated).unwrap_err().code,
            ErrorCode::BudgetExceeded
        );
    }
    assert_eq!(
        decode(&vec![0; MAX_SCENE_BYTES + 1]).unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn rejects_nonfinite_geometry_and_font_sizes_from_wire() {
    let mut scene = empty();
    scene.nodes.push(node(1, text("ok".into())));
    let bytes = scene.validate().unwrap().encode();
    for offset in [34, 38, 42, 46, 55] {
        for bits in [
            f32::NAN.to_bits(),
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
            0xff80_0001,
        ] {
            let mut mutated = bytes.clone();
            mutated[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
            let error = decode(&mutated).unwrap_err();
            assert_eq!(
                error,
                SceneError {
                    code: ErrorCode::NonFinite,
                    context: Context::Node(0)
                }
            );
        }
    }
}

#[test]
fn node_and_resource_identity_cannot_alias_or_dangle() {
    let mut scene = fixture();
    scene.nodes[2].id = 1;
    assert_eq!(scene.validate().unwrap_err().code, ErrorCode::DuplicateNode);
    let mut scene = fixture();
    scene.nodes[0].id = 0;
    assert_eq!(scene.validate().unwrap_err().code, ErrorCode::InvalidNode);
    let mut scene = fixture();
    scene.resources.push(scene.resources[0].clone());
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::DuplicateResource
    );
    let mut scene = fixture();
    scene.resources[0].id = 0;
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::InvalidResource
    );
    let mut scene = fixture();
    scene.resources.clear();
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::UnknownResource
    );
}

#[test]
fn image_dimension_and_aggregate_budgets_include_unused_resources() {
    let mut scene = empty();
    scene.resources = (1..=2)
        .map(|id| ImageResource {
            id,
            width: 4096,
            height: 4096,
        })
        .collect();
    scene.clone().validate().unwrap(); // Exactly 128 MiB declared RGBA8.
    scene.resources.push(ImageResource {
        id: 3,
        width: 1,
        height: 1,
    });
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
    for dimension in [0, 4097, u32::MAX] {
        let mut scene = empty();
        scene.resources.push(ImageResource {
            id: 1,
            width: dimension,
            height: 1,
        });
        assert_eq!(
            scene.validate().unwrap_err().code,
            ErrorCode::InvalidResource
        );
    }
}

#[test]
fn text_limits_count_utf8_bytes_and_aggregate_before_use() {
    let mut scene = empty();
    scene
        .nodes
        .push(node(1, text("é".repeat(MAX_TEXT_BYTES / 2))));
    scene.clone().validate().unwrap();
    if let Content::Text { text, .. } = &mut scene.nodes[0].content {
        text.push('x');
    }
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
    let mut scene = empty();
    scene.nodes = (1..=64)
        .map(|id| node(id, text("a".repeat(MAX_TEXT_BYTES))))
        .collect();
    let bytes = scene.clone().validate().unwrap().encode();
    assert!(bytes.len() < MAX_SCENE_BYTES);
    decode(&bytes).unwrap();
    scene.nodes.push(node(65, text("x".into())));
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn invalid_utf8_and_control_text_are_rejected_without_echoing_payload() {
    let mut scene = empty();
    scene.nodes.push(node(1, text("ok".into())));
    let mut bytes = scene.validate().unwrap().encode();
    bytes[63] = 0xff;
    assert_eq!(decode(&bytes).unwrap_err().code, ErrorCode::InvalidText);
    for control in ['\0', '\r', '\t', '\u{1b}', '\u{7f}'] {
        let mut scene = empty();
        scene
            .nodes
            .push(node(1, text(format!("secret{control}payload"))));
        let error = scene.validate().unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidText);
        assert!(!error.to_string().contains("secret"));
    }
}

#[test]
fn scene_and_geometry_boundaries_are_enforced() {
    for (width, height) in [(0, 1), (8193, 1), (8192, 8192), (u32::MAX, u32::MAX)] {
        let mut scene = empty();
        scene.width = width;
        scene.height = height;
        assert_eq!(scene.validate().unwrap_err().code, ErrorCode::InvalidCanvas);
    }
    let mut scene = empty();
    scene.width = 8192;
    scene.height = 4096;
    scene.validate().unwrap();
    let mut scene = fixture();
    scene.nodes[0].bounds.width = -1.0;
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::InvalidGeometry
    );
    let mut scene = fixture();
    scene.nodes[0].bounds.x = 32769.0;
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::InvalidGeometry
    );
    let mut scene = fixture();
    scene.nodes[0].bounds.width = 0.0;
    scene.validate().unwrap();
}

#[test]
fn node_and_resource_count_boundaries_are_enforced() {
    let mut scene = empty();
    scene.nodes = (1..=MAX_NODES as u32)
        .map(|id| {
            node(
                id,
                Content::Rectangle {
                    color: Color([0; 4]),
                },
            )
        })
        .collect();
    scene.clone().validate().unwrap();
    scene.nodes.push(node(
        MAX_NODES as u32 + 1,
        Content::Rectangle {
            color: Color([0; 4]),
        },
    ));
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
    let mut scene = empty();
    scene.resources = (1..=MAX_RESOURCES as u32)
        .map(|id| ImageResource {
            id,
            width: 1,
            height: 1,
        })
        .collect();
    scene.clone().validate().unwrap();
    scene.resources.push(ImageResource {
        id: MAX_RESOURCES as u32 + 1,
        width: 1,
        height: 1,
    });
    assert_eq!(
        scene.validate().unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn deterministic_byte_mutations_never_bypass_revalidation() {
    let bytes = fixture().validate().unwrap().encode();
    let mut seed = 0x1234_5678u32;
    for _ in 0..1024 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let mut mutated = bytes.clone();
        mutated[seed as usize % bytes.len()] ^= (seed >> 24) as u8;
        if let Ok(scene) = decode(&mutated) {
            assert_eq!(decode(&scene.encode()).unwrap(), scene);
            assert!(scene.encode().len() <= MAX_SCENE_BYTES);
        }
    }
}

#[test]
fn combined_maximum_budgets_fit_the_binary_envelope() {
    let mut scene = empty();
    scene.resources = (1..=MAX_RESOURCES as u32)
        .map(|id| ImageResource {
            id,
            width: 1,
            height: 1,
        })
        .collect();
    scene.nodes = (1..=MAX_NODES as u32)
        .map(|id| {
            node(
                id,
                if id <= 64 {
                    text("a".repeat(MAX_TEXT_BYTES))
                } else {
                    Content::Image { resource: 1 }
                },
            )
        })
        .collect();
    let scene = scene.validate().unwrap();
    let bytes = scene.encode();
    assert!(bytes.len() <= MAX_SCENE_BYTES);
    assert_eq!(decode(&bytes).unwrap(), scene);
}
