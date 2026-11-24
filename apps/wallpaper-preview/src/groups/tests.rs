use super::*;
use wallpaper_scene::{
    Color as SceneColor, Content,
    hierarchy::{self, Hierarchy, Layer, LayerContent, Transform},
};
fn original() -> hierarchy::ValidatedHierarchy {
    let compiled = wallpaper_compiler::compile_groups(
        &wallpaper_compiler::fixtures::groups_package().unwrap(),
        wallpaper_compiler::Viewport {
            width: 1920,
            height: 1080,
            background: SceneColor([12, 17, 22, 255]),
        },
    )
    .unwrap();
    hierarchy::decode(&compiled.scene().encode()).unwrap()
}
fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn group(id: u32, parent: u32, opacity: f32) -> Layer {
    Layer {
        id,
        parent,
        bounds: rect(0.0, 0.0, 64.0, 64.0),
        content: LayerContent::Group {
            transform: Transform::default(),
            opacity,
            clip: true,
        },
    }
}
fn draw(id: u32, parent: u32, content: Content) -> Layer {
    Layer {
        id,
        parent,
        bounds: rect(0.0, 0.0, 32.0, 32.0),
        content: LayerContent::Draw(content),
    }
}
fn scene(layers: Vec<Layer>) -> Hierarchy {
    Hierarchy {
        version: hierarchy::VERSION,
        width: 64,
        height: 64,
        background: SceneColor([0, 0, 0, 255]),
        resources: vec![],
        layers,
    }
}
/// One-pixel reference for rectangle/group regions, independent of GPU shaders.
fn sample(plan: &Plan, x: f32, y: f32) -> [f32; 4] {
    let bg = plan::linear_color(plan.background.0);
    let mut targets = vec![[0.0; 4]; plan.targets];
    targets[0] = [bg[0] * bg[3], bg[1] * bg[3], bg[2] * bg[3], bg[3]];
    for op in &plan.ops {
        let q = &plan.quads[op.quad];
        let contains = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        if !contains(q.bounds) || !contains(q.clip) {
            continue;
        }
        let source = match op.paint {
            Paint::Rectangle => [
                q.color[0] * q.color[3],
                q.color[1] * q.color[3],
                q.color[2] * q.color[3],
                q.color[3],
            ],
            Paint::Premultiplied(Texture::Target(index)) => {
                assert_ne!(op.target, index);
                targets[index].map(|c| c * q.opacity)
            }
            _ => panic!("Reference sample overlaps a text/image region"),
        };
        let dest = targets[op.target];
        targets[op.target] = std::array::from_fn(|i| source[i] + dest[i] * (1.0 - source[3]));
    }
    targets[0]
}

#[test]
fn original_plan_isolates_opacity_preserves_order_and_reserves_textures() {
    let plan = Plan::new(&original(), 8192).unwrap();
    assert_eq!(
        (
            plan.targets,
            plan.ops.len(),
            plan.texts.len(),
            plan.texture_bytes
        ),
        (2, 6, 1, 16_691_456)
    );
    assert_eq!(plan.texts[0].pixels, [400, 64]);
    assert_eq!(plan.texts[0].raster_scale, 2.0);
    assert!(plan.ops[..5].iter().all(|op| op.target == 1));
    assert_eq!(plan.ops[5].target, 0);
    assert_eq!(plan.ops[5].paint, Paint::Premultiplied(Texture::Target(1)));
    assert_eq!(plan.quads[plan.ops[5].quad].opacity, 0.5);
    assert!(plan.quads[..5].iter().all(|quad| quad.opacity == 1.0));
    let pixel = sample(&plan, 210.5, 220.5);
    let bg = plan::linear_color(plan.background.0);
    for i in 0..3 {
        let expected = bg[i] * 0.5 + if i == 2 { 0.5 } else { 0.0 };
        assert!((pixel[i] - expected).abs() < 0.000001);
    }
    assert_eq!(pixel[3], 1.0);
}

#[test]
fn nested_groups_composite_child_before_parent_and_never_sample_destination() {
    let s = scene(vec![
        group(1, 0, 0.5),
        group(2, 1, 0.5),
        draw(
            3,
            2,
            Content::Rectangle {
                color: SceneColor([255; 4]),
            },
        ),
    ])
    .validate()
    .unwrap();
    let plan = Plan::new(&s, 8192).unwrap();
    assert_eq!(plan.targets, 3);
    assert_eq!(
        plan.ops.iter().map(|op| op.target).collect::<Vec<_>>(),
        [2, 1, 0]
    );
    assert_eq!(plan.ops[1].paint, Paint::Premultiplied(Texture::Target(2)));
    assert_eq!(plan.ops[2].paint, Paint::Premultiplied(Texture::Target(1)));
    assert_eq!(sample(&plan, 10.5, 10.5), [0.25, 0.25, 0.25, 1.0]);
    let mut raw = s.scene().clone();
    if let LayerContent::Group { opacity, .. } = &mut raw.layers[0].content {
        *opacity = 0.0;
    }
    let invisible = Plan::new(&raw.validate().unwrap(), 8192).unwrap();
    assert_eq!(invisible.targets, 3);
    assert_eq!(sample(&invisible, 10.5, 10.5), [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn opaque_groups_do_not_require_an_extra_surface() {
    let mut raw = original().scene().clone();
    if let LayerContent::Group { opacity, .. } = &mut raw.layers[0].content {
        *opacity = 1.0;
    }
    let plan = Plan::new(&raw.validate().unwrap(), 8192).unwrap();
    assert_eq!((plan.targets, plan.ops.len()), (1, 5));
    assert!(plan.ops.iter().all(|op| op.target == 0));
}

#[test]
fn fractional_clips_and_letterboxing_keep_exact_shader_coordinates() {
    assert_eq!(
        plan::scissor(rect(1.25, 2.75, 4.5, 5.5), 100, 100),
        Some([1, 2, 5, 7])
    );
    assert_eq!(
        plan::scissor(rect(-3.0, -5.0, 10.0, 12.0), 100, 100),
        Some([0, 0, 7, 7])
    );
    assert_eq!(plan::scissor(rect(200.0, 0.0, 10.0, 10.0), 100, 100), None);
    assert_eq!(plan::scissor(rect(1.0, 1.0, 0.0, 10.0), 100, 100), None);
    assert_eq!(
        plan::fit([1920, 1080], [960, 600]),
        rect(0.0, 30.0, 960.0, 540.0)
    );
    let q = Quad {
        bounds: rect(1.0, 2.0, 3.0, 4.0),
        clip: rect(1.25, 2.75, 4.5, 5.5),
        color: [1.0; 4],
        opacity: 0.5,
        uv: [0.75, 1.0],
    };
    let mut record = [0; RECORD];
    write_record(&mut record, &q, [100, 100]);
    let value = |offset| f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
    assert_eq!(
        [value(32), value(36), value(40), value(44)],
        [1.25, 2.75, 5.75, 8.25]
    );
    assert_eq!([value(64), value(68), value(72)], [0.5, 0.75, 1.0]);
}

#[test]
fn text_rasterization_handles_anisotropic_scale_and_rejects_excessive_work() {
    let text = || Content::Text {
        text: "x".into(),
        font_size: 16.0,
        color: SceneColor([255; 4]),
    };
    let mut g = group(1, 0, 1.0);
    if let LayerContent::Group { transform, .. } = &mut g.content {
        transform.scale = [2.0, 3.0];
    }
    let plan = Plan::new(
        &scene(vec![g, draw(2, 1, text())]).validate().unwrap(),
        8192,
    )
    .unwrap();
    assert_eq!(plan.texts[0].pixels, [96, 96]);
    assert_eq!(plan.texts[0].raster_scale, 3.0);
    assert_eq!(plan.quads[0].bounds, rect(0.0, 0.0, 64.0, 96.0));
    let too_many = (1..=129).map(|id| draw(id, 0, text())).collect();
    assert!(Plan::new(&scene(too_many).validate().unwrap(), 8192).is_err());
    let mut huge = draw(1, 0, text());
    huge.bounds = rect(0.0, 0.0, 4096.0, 4096.0);
    Plan::new(&scene(vec![huge.clone()]).validate().unwrap(), 8192).unwrap();
    let mut second = huge.clone();
    second.id = 2;
    assert!(Plan::new(&scene(vec![huge, second]).validate().unwrap(), 8192).is_err());
    let mut big_font = draw(2, 1, text());
    big_font.content = LayerContent::Draw(Content::Text {
        text: "x".into(),
        font_size: 512.0,
        color: SceneColor([255; 4]),
    });
    let mut g = group(1, 0, 1.0);
    if let LayerContent::Group { transform, .. } = &mut g.content {
        transform.scale = [2.0; 2];
    }
    assert!(Plan::new(&scene(vec![g, big_font]).validate().unwrap(), 8192).is_err());
}

#[test]
fn total_texture_and_device_limits_fail_before_gpu_allocation() {
    assert!(Plan::new(&original(), 1024).is_err());
    let mut big = scene(vec![group(1, 0, 0.5)]);
    big.width = 8192;
    big.height = 4096;
    let plan = Plan::new(&big.clone().validate().unwrap(), 8192).unwrap();
    assert_eq!(plan.texture_bytes, plan::MAX_TEXTURE_BYTES);
    big.resources.push(wallpaper_scene::ImageResource {
        id: 1,
        width: 1,
        height: 1,
    });
    assert!(Plan::new(&big.validate().unwrap(), 8192).is_err());
}

#[test]
fn group_shader_parses_and_validates_without_a_gpu_or_window() {
    let module = wgpu::naga::front::wgsl::parse_str(include_str!("../groups.wgsl")).unwrap();
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    for name in ["vs", "fs_rect", "fs_straight", "fs_premultiplied"] {
        assert!(module.entry_points.iter().any(|entry| entry.name == name));
    }
}

#[test]
fn preview_fixture_deliberately_overflows_text_and_png_beyond_the_parent_clip() {
    let compiled = wallpaper_compiler::compile_groups(
        &wallpaper_compiler::fixtures::preview_groups_package().unwrap(),
        wallpaper_compiler::Viewport {
            width: 1920,
            height: 1080,
            background: SceneColor([12, 17, 22, 255]),
        },
    )
    .unwrap();
    let scene = hierarchy::decode(&compiled.scene().encode()).unwrap();
    wallpaper_media::validate_hierarchy_assets(&scene, compiled.images()).unwrap();
    let plan = Plan::new(&scene, 8192).unwrap();
    assert_eq!(
        (
            plan.targets,
            plan.ops.len(),
            plan.texts.len(),
            plan.texture_bytes
        ),
        (2, 6, 1, 16_804_096)
    );
    assert_eq!(plan.texts[0].pixels, [840, 64]);
    let text = plan
        .ops
        .iter()
        .find(|op| matches!(op.paint, Paint::Premultiplied(Texture::Text(_))))
        .unwrap();
    let text = &plan.quads[text.quad];
    assert!(text.bounds.x + text.bounds.width > text.clip.x + text.clip.width);
    let png = plan
        .ops
        .iter()
        .find(|op| matches!(op.paint, Paint::Straight(Texture::Image(_))))
        .unwrap();
    let png = &plan.quads[png.quad];
    assert_eq!(png.bounds, rect(650.0, 484.0, 128.0, 128.0));
    assert_eq!(png.clip, rect(90.0, 100.0, 640.0, 480.0));
    assert!(png.bounds.x + png.bounds.width > png.clip.x + png.clip.width);
    assert!(png.bounds.y + png.bounds.height > png.clip.y + png.clip.height);
}

fn effects() -> (wallpaper_compiler::CompiledGroups, Plan) {
    let compiled = wallpaper_compiler::compile_groups(
        &wallpaper_compiler::fixtures::effect_groups_package().unwrap(),
        wallpaper_compiler::Viewport {
            width: 1920,
            height: 1080,
            background: SceneColor([12, 17, 22, 255]),
        },
    )
    .unwrap();
    let decoded = hierarchy::decode(&compiled.scene().encode()).unwrap();
    wallpaper_media::validate_hierarchy_assets(&decoded, compiled.images()).unwrap();
    assert_eq!(&decoded, compiled.scene());
    let plan = Plan::new(&decoded, 8192).unwrap();
    (compiled, plan)
}
fn resolved(compiled: &wallpaper_compiler::CompiledGroups, id: &str) -> hierarchy::ResolvedLayer {
    compiled.scene().resolved()[compiled.symbols()[id] as usize - 1]
}

#[test]
fn effect_fixture_preserves_nested_opacity_and_distinguishes_one_level_badge() {
    let (compiled, plan) = effects();
    assert_eq!(
        (
            compiled.scene().scene().layers.len(),
            compiled.scene().commands().len()
        ),
        (44, 66)
    );
    assert_eq!(
        (
            plan.targets,
            plan.ops.len(),
            plan.texts.len(),
            plan.texture_bytes
        ),
        (3, 22, 7, 25_643_936)
    );
    let composites: Vec<_> = plan
        .ops
        .iter()
        .filter_map(|op| match op.paint {
            Paint::Premultiplied(Texture::Target(source)) => {
                Some((source, op.target, plan.quads[op.quad].opacity))
            }
            _ => None,
        })
        .collect();
    assert_eq!(composites, [(2, 1, 0.5), (1, 0, 0.5)]);
    let bg = plan::linear_color(plan.background.0);
    for (x, y, color, factor) in [
        (190.5, 270.5, [0.0, 0.0, 1.0], 0.25),
        (110.5, 190.5, [1.0, 0.0, 0.0], 0.25),
        (410.5, 190.5, [1.0; 3], 0.5),
    ] {
        let pixel = sample(&plan, x, y);
        for i in 0..3 {
            assert!((pixel[i] - (bg[i] * (1.0 - factor) + color[i] * factor)).abs() < 0.000001);
        }
        assert_eq!(pixel[3], 1.0);
    }
    assert_eq!(sample(&plan, 190.5, 410.5), bg); // Blue content ends at the inner clip, not the outer one.
}

#[test]
fn effect_fixture_keeps_nonuniform_text_and_image_transforms_separate_from_control() {
    let (compiled, plan) = effects();
    assert_eq!(
        resolved(&compiled, "stretchedText").bounds,
        rect(640.0, 180.0, 270.0, 30.0)
    );
    assert_eq!(
        resolved(&compiled, "stretchedImage").bounds,
        rect(640.0, 225.0, 240.0, 90.0)
    );
    assert_eq!(
        resolved(&compiled, "controlText").bounds,
        rect(640.0, 370.0, 400.0, 40.0)
    );
    let scaled = plan
        .texts
        .iter()
        .find(|text| text.text == "Wide / short" && text.raster_scale == 1.5)
        .unwrap();
    assert_eq!(scaled.pixels, [270, 60]);
    let control = plan
        .texts
        .iter()
        .find(|text| text.text == "Wide / short" && text.raster_scale == 1.0)
        .unwrap();
    assert_eq!(control.pixels, [400, 40]);
    assert_eq!(compiled.images()[&1].pixels().len(), 256);
    assert_eq!(&compiled.images()[&1].pixels()[252..], &[230, 64, 96, 128]);
}

#[test]
fn effect_fixture_requires_fractional_pixel_center_tests_and_empty_clip_culling() {
    let (compiled, plan) = effects();
    let low = resolved(&compiled, "lowFill").clip;
    let high = resolved(&compiled, "highFill").clip;
    assert_eq!(low, rect(1240.25, 180.25, 100.0, 100.0));
    assert_eq!(high, rect(1400.75, 180.75, 100.0, 100.0));
    assert_eq!(plan::scissor(low, 1920, 1080), Some([1240, 180, 101, 101]));
    assert_eq!(plan::scissor(high, 1920, 1080), Some([1400, 180, 101, 101]));
    let panel = plan::linear_color([31, 42, 51, 255]);
    for (x, y, expected) in [
        (1240.5, 181.5, [1.0; 4]),
        (1400.5, 181.5, panel),
        (1340.5, 200.5, panel),
        (1500.5, 200.5, [1.0; 4]),
        (1250.5, 180.5, [1.0; 4]),
        (1410.5, 180.5, panel),
        (1250.5, 280.5, panel),
        (1410.5, 280.5, [1.0; 4]),
    ] {
        assert_eq!(sample(&plan, x, y), expected);
    }
    let empty = resolved(&compiled, "emptyFill").clip;
    assert_eq!(empty.width, 0.0);
    assert_eq!(plan::scissor(empty, 1920, 1080), None);
    assert_eq!(sample(&plan, 1560.5, 200.5), panel);
}
