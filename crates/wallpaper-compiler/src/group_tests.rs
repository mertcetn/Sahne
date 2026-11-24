use super::*;
use wallpaper_format::Package;
use wallpaper_scene::{
    Color, Content, Rect,
    hierarchy::{self, LayerContent, Transform},
};

fn viewport() -> Viewport {
    Viewport {
        width: 1920,
        height: 1080,
        background: Color([12, 17, 22, 255]),
    }
}
fn package(html: &str, css: &str) -> Package {
    let original = fixtures::groups_package().unwrap();
    Package::from_entries(
        original
            .files()
            .map(|file| {
                (
                    file.path().as_str().to_owned(),
                    match file.path().as_str() {
                        "index.html" => html.as_bytes().to_vec(),
                        "style.css" => css.as_bytes().to_vec(),
                        _ => file.bytes().to_vec(),
                    },
                )
            })
            .collect(),
    )
    .unwrap()
}
fn effect(scene: &CompiledGroups, index: usize) -> (Transform, f32, bool) {
    let LayerContent::Group {
        transform,
        opacity,
        clip,
    } = scene.scene().scene().layers[index].content
    else {
        panic!("Expected group")
    };
    (transform, opacity, clip)
}

#[test]
fn original_group_sources_preserve_subtrees_alpha_and_independent_image_assets() {
    let compiled = compile_groups(&fixtures::groups_package().unwrap(), viewport()).unwrap();
    let bytes = compiled.scene().encode();
    let decoded = hierarchy::decode(&bytes).unwrap();
    assert_eq!(&decoded, compiled.scene());
    wallpaper_media::validate_hierarchy_assets(&decoded, compiled.images()).unwrap();
    assert_eq!(
        (
            decoded.scene().layers.len(),
            decoded.commands().len(),
            bytes.len()
        ),
        (10, 15, 439)
    );
    assert_eq!(compiled.symbols()["group"], 1);
    assert_eq!(compiled.symbols()["blue"], 5);
    assert_eq!(compiled.symbols()["pattern"], 9);
    assert_eq!(
        decoded.resolved()[4].bounds,
        Rect {
            x: 190.0,
            y: 200.0,
            width: 200.0,
            height: 200.0
        }
    );
    assert_eq!(
        decoded.resolved()[9].bounds,
        Rect {
            x: 530.0,
            y: 420.0,
            width: 128.0,
            height: 128.0
        }
    );
    assert_eq!(
        effect(&compiled, 0),
        (
            Transform {
                translate: [10.0, 20.0],
                scale: [2.0, 2.0]
            },
            0.5,
            true
        )
    );
    assert_eq!(effect(&compiled, 2), (Transform::default(), 1.0, false));
    assert_eq!(
        decoded.scene().layers[3].content,
        LayerContent::Draw(Content::Rectangle {
            color: Color([255, 0, 0, 255])
        })
    );
    assert_eq!(compiled.images()[&1].pixels().len(), 256);
    assert_eq!(&compiled.images()[&1].pixels()[252..], &[230, 64, 96, 128]);
    assert!(compiled.script().is_none());
}

#[test]
fn v1_mode_stays_strict_and_original_examples_keep_their_encoded_sizes() {
    assert_eq!(
        compile(&fixtures::groups_package().unwrap(), viewport())
            .unwrap_err()
            .code,
        ErrorCode::UnsupportedProperty
    );
    for (fixture, size) in [
        (fixtures::static_package().unwrap(), 123),
        (fixtures::image_package().unwrap(), 136),
    ] {
        assert_eq!(
            compile(&fixture, viewport())
                .unwrap()
                .scene()
                .encode()
                .len(),
            size
        );
        let grouped = compile_groups(&fixture, viewport()).unwrap();
        let decoded = hierarchy::decode(&grouped.scene().encode()).unwrap();
        wallpaper_media::validate_hierarchy_assets(&decoded, grouped.images()).unwrap();
    }
}

#[test]
fn group_properties_cascade_independently_and_do_not_inherit() {
    let html = "<link rel='stylesheet' href='style.css'><div id='root' class='box' style='width:100px;height:100px;opacity:0.75'><span id='child' style='width:20px;height:20px'>x</span></div>";
    let css = "#root{opacity:0.5;transform:translate(5px,7px) scale(2,3);transform-origin:0 0;overflow:clip;color:#f00;font-size:24px}.box{opacity:0.25;transform:scale(4);overflow:visible}";
    let compiled = compile_groups(&package(html, css), viewport()).unwrap();
    assert_eq!(
        effect(&compiled, 0),
        (
            Transform {
                translate: [5.0, 7.0],
                scale: [2.0, 3.0]
            },
            0.75,
            true
        )
    );
    assert_eq!(effect(&compiled, 2), (Transform::default(), 1.0, false));
    assert_eq!(
        compiled.scene().scene().layers[3].content,
        LayerContent::Draw(Content::Text {
            text: "x".into(),
            font_size: 24.0,
            color: Color([255, 0, 0, 255])
        })
    );
}

#[test]
fn transform_function_allowlist_and_origin_requirement_are_explicit() {
    for (value, expected) in [
        ("none", Transform::default()),
        (
            "translate(-2px,0)",
            Transform {
                translate: [-2.0, 0.0],
                scale: [1.0, 1.0],
            },
        ),
        (
            "scale(0)",
            Transform {
                translate: [0.0, 0.0],
                scale: [0.0, 0.0],
            },
        ),
        (
            "translate(1px,2px) scale(3,4)",
            Transform {
                translate: [1.0, 2.0],
                scale: [3.0, 4.0],
            },
        ),
    ] {
        let html = format!(
            "<div style='width:10px;height:10px;transform-origin:0px 0px;transform:{value}'></div>"
        );
        assert_eq!(
            effect(&compile_groups(&package(&html, ""), viewport()).unwrap(), 0).0,
            expected
        );
    }
    let html = "<div style='width:10px;height:10px;transform:scale(1)'></div>";
    assert_eq!(
        compile_groups(&package(html, ""), viewport())
            .unwrap_err()
            .code,
        ErrorCode::MissingTransformOrigin
    );
    let html = "<div style='width:10px;height:10px;transform:none'></div>";
    compile_groups(&package(html, ""), viewport()).unwrap();
}

#[test]
fn unsupported_effect_values_fail_even_in_unmatched_rules_with_css_locations() {
    let html = "<link rel='stylesheet' href='style.css'><div style='width:1px;height:1px'></div>";
    for declaration in [
        "transform:rotate(20deg)",
        "transform:scale(2) translate(1px,2px)",
        "transform:translate(1px,2px)translate(3px,4px)",
        "transform:scale(-1)",
        "transform:scale(17)",
        "transform:scale(NaN)",
        "transform:translate(1%,2px)",
        "transform:translate(1px)",
        "transform:scale(1,2,3)",
        "transform:scale(1) scale(2)",
        "transform:scale(1e2)",
        "transform-origin:50% 50%",
        "transform-origin:0 0 0",
        "overflow:hidden",
        "overflow:auto",
        "opacity:50%",
        "opacity:1.01",
        "opacity:NaN",
        "opacity:1e-1",
        "opacity:0.5!important",
        "z-index:1",
        "filter:blur(1px)",
    ] {
        let css = format!("\n.unmatched{{{declaration}}}");
        let error = compile_groups(&package(html, &css), viewport()).unwrap_err();
        assert_eq!(error.path.as_str(), "style.css");
        assert_eq!(error.line, 2);
    }
}

#[test]
fn ancestor_effect_failures_map_to_original_html_elements() {
    let html = "<div style='width:10px;height:10px;transform-origin:0 0;transform:scale(16)'>\n<div style='width:10px;height:10px;transform-origin:0 0;transform:scale(16)'></div></div>";
    let error = compile_groups(&package(html, ""), viewport()).unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::SceneValidation(wallpaper_scene::ErrorCode::InvalidEffect)
    );
    assert_eq!((error.line, error.column), (2, 1));
    let html = (0..9)
        .map(|_| "<div style='width:10px;height:10px;opacity:0.5'></div>")
        .collect::<String>();
    let error = compile_groups(&package(&html, ""), viewport()).unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::SceneValidation(wallpaper_scene::ErrorCode::BudgetExceeded)
    );
    assert!(error.span.start > 0);
}

#[test]
fn generated_group_and_primitive_layers_share_one_output_budget() {
    let element = "<div style='width:1px;height:1px'></div>";
    let compiled = compile_groups(&package(&element.repeat(2048), ""), viewport()).unwrap();
    assert_eq!(compiled.scene().scene().layers.len(), 4096);
    assert_eq!(
        compile_groups(&package(&element.repeat(2049), ""), viewport())
            .unwrap_err()
            .code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn deterministic_group_source_mutations_never_skip_scene_or_asset_validation() {
    let original = fixtures::groups_package().unwrap();
    let html = original
        .files()
        .find(|f| f.path().as_str() == "index.html")
        .unwrap()
        .utf8()
        .unwrap();
    let css = original
        .files()
        .find(|f| f.path().as_str() == "style.css")
        .unwrap()
        .utf8()
        .unwrap();
    for seed in 0..256usize {
        let mut changed = css.as_bytes().to_vec();
        changed[(seed * 19 + 7) % css.len()] ^= 1 << (seed % 7);
        if let Ok(text) = std::str::from_utf8(&changed)
            && let Ok(compiled) = compile_groups(&package(html, text), viewport())
        {
            let decoded = hierarchy::decode(&compiled.scene().encode()).unwrap();
            wallpaper_media::validate_hierarchy_assets(&decoded, compiled.images()).unwrap();
            assert_eq!(&decoded, compiled.scene());
        }
    }
}
