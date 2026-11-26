use super::*;
fn path() -> RelativePath {
    RelativePath::parse("fixture.html").unwrap()
}
#[test]
fn original_fragment_preserves_parentage_identifiers_and_literal_text() {
    let path = path();
    let document = html::parse(Source { path: &path, text: "<link rel='stylesheet' href='style.css'><div id='root'><span class='label'>A &amp; B&#33;\r\n日本語</span><img src='image.png'/></div>" }).unwrap();
    assert_eq!(document.elements.len(), 3);
    assert_eq!(document.elements[1].parent, Some(0));
    assert_eq!(document.elements[1].text, "A & B!\n日本語");
    assert_eq!(document.stylesheets[0].path.as_str(), "style.css");
}
#[test]
fn rejects_browser_recovery_and_mixed_content() {
    let path = path();
    for text in [
        "<div>",
        "</div>",
        "<div><span></div></span>",
        "<span><span>x</span></span>",
        "<div>mixed</div>",
        "<script>x</script>",
        "<html></html>",
        "<div/>",
        "<div id=x></div>",
        "<img src='http://host/a.png'>",
    ] {
        assert!(
            html::parse(Source { path: &path, text }).is_err(),
            "source={text}"
        );
    }
}
#[test]
fn duplicate_ids_and_attributes_and_events_are_rejected() {
    let path = path();
    for (text, code) in [
        (
            "<div id='a'></div><span id='a'></span>",
            ErrorCode::DuplicateId,
        ),
        ("<div id='a' id='b'></div>", ErrorCode::DuplicateAttribute),
        ("<div onclick='bad'></div>", ErrorCode::UnsupportedAttribute),
    ] {
        assert_eq!(
            html::parse(Source { path: &path, text }).unwrap_err().code,
            code
        );
    }
}
#[test]
fn css_parses_simple_selectors_colors_and_finite_pixels() {
    let path = path();
    let stylesheet = css::parse(Source { path: &path, text: "/* original */ .label { color: #abc; width: 100px; } #root { left: -2.5px; background-color: #10203080; }" }).unwrap();
    assert_eq!(stylesheet.rules.len(), 2);
    assert!(
        stylesheet.rules[1].selector.specificity() > stylesheet.rules[0].selector.specificity()
    );
    assert_eq!(
        stylesheet.rules[0].declarations[0].property,
        css::Property::Color(wallpaper_scene::Color([170, 187, 204, 255]))
    );
}
#[test]
fn unsupported_css_and_nonfinite_or_invalid_numbers_fail_closed() {
    let path = path();
    for text in [
        "div { width:1px }",
        "#a,.b { width:1px }",
        "@import 'x';",
        ".a {width:NaNpx}",
        ".a {width:1e9px}",
        ".a {width:-1px}",
        ".a {width:20%}",
        ".a {color:#ggg}",
        ".a {width:1px !important}",
        ".a {display:flex}",
        ".a {width:calc(1px)}",
        ".a {width:1px",
        ".a {width:1px; /*",
        ".a {content:'/*x*/'}",
    ] {
        assert!(
            css::parse(Source { path: &path, text }).is_err(),
            "source={text}"
        );
    }
}
#[test]
fn inline_errors_refer_to_original_html_bytes_without_echoing_source() {
    let path = path();
    let text = "<div>\n<span style='width:NaNpx'></span></div>";
    let document = html::parse(Source { path: &path, text }).unwrap();
    let (_, span) = document.elements[1].inline_style.as_ref().unwrap();
    let error = css::declarations(Source { path: &path, text }, *span).unwrap_err();
    assert_eq!(error.line, 2);
    assert_eq!(error.span.start, text.find("width").unwrap());
    assert!(!error.to_string().contains("NaN"));
}

fn package(html: &str, css: &str) -> wallpaper_format::Package {
    wallpaper_format::Package::from_entries(vec![
        ("manifest.json".into(), br#"{"formatVersion":1,"apiVersion":1,"id":"compiler.fixture","name":"Original fixture","entry":"index.html","permissions":[],"preferredFps":30}"#.to_vec()),
        ("index.html".into(), html.as_bytes().to_vec()), ("style.css".into(), css.as_bytes().to_vec()),
    ]).unwrap()
}
fn viewport() -> Viewport {
    Viewport {
        width: 1920,
        height: 1080,
        background: wallpaper_scene::Color([0, 0, 0, 255]),
    }
}

#[test]
fn script_targets_prefer_span_text_and_skip_unbound_nodes() {
    let package = package(
        r#"<span id="label" style="width:100px;height:24px;background-color:#222222">Text</span>
           <div id="panel" style="width:100px;height:24px;background-color:#333333"></div>
           <div style="width:100px;height:24px;background-color:#444444"></div>"#,
        "",
    );
    let compiled = compile(&package, viewport()).unwrap();
    let label = &compiled.symbols()["label"];
    assert_eq!(label.len(), 2);
    assert_eq!(
        compiled.script_targets(),
        std::collections::BTreeMap::from([
            ("label".to_owned(), label[1]),
            ("panel".to_owned(), compiled.symbols()["panel"][0]),
        ])
    );
}

#[test]
fn original_static_sources_compile_to_expected_independently_validated_scene() {
    let package = fixtures::static_package().unwrap();
    let compiled = compile(&package, viewport()).unwrap();
    let scene = compiled.scene().scene();
    assert_eq!(scene.nodes.len(), 3);
    assert_eq!(scene.nodes[1].bounds.x, 104.0);
    assert_eq!(scene.nodes[1].bounds.y, 104.0);
    assert_eq!(scene.nodes[2].bounds.y, 216.0);
    assert_eq!(compiled.symbols()["title"], [2]);
    assert_eq!(
        scene.nodes[1].content,
        wallpaper_scene::Content::Text {
            text: "Sahne / M3".into(),
            font_size: 28.0,
            color: wallpaper_scene::Color([155, 220, 202, 255])
        }
    );
    assert_eq!(
        &wallpaper_scene::decode(&compiled.scene().encode()).unwrap(),
        compiled.scene()
    );
}

#[test]
fn specificity_source_order_and_inline_styles_apply_per_property() {
    let css = "#box {width:100px} .box {width:20px; width:30px; height:40px} .box {width:50px}";
    let html = "<link rel='stylesheet' href='style.css'><div id='box' class='box'></div>";
    let compiled = compile(&package(html, css), viewport()).unwrap();
    assert_eq!(compiled.scene().scene().nodes[0].bounds.width, 100.0);
    assert_eq!(compiled.scene().scene().nodes[0].bounds.height, 40.0);
    let html = html.replace("class='box'", "class='box' style='width:120px'");
    assert_eq!(
        compile(&package(&html, css), viewport())
            .unwrap()
            .scene()
            .scene()
            .nodes[0]
            .bounds
            .width,
        120.0
    );
    let html = html
        .replace("id='box'", "id='other'")
        .replace(" style='width:120px'", "");
    assert_eq!(
        compile(&package(&html, css), viewport())
            .unwrap()
            .scene()
            .scene()
            .nodes[0]
            .bounds
            .width,
        50.0
    );
}

#[test]
fn inherited_values_do_not_retain_parent_selector_specificity() {
    let html =
        "<link rel='stylesheet' href='style.css'><div id='root'><span class='child'>x</span></div>";
    let css = "#root {width:200px;height:100px;left:10px;top:20px;color:#f00;font-size:40px} .child {width:40px;height:40px;left:5px;top:7px;color:#0f0}";
    let compiled = compile(&package(html, css), viewport()).unwrap();
    assert_eq!(compiled.scene().scene().nodes[1].bounds.x, 15.0);
    assert_eq!(compiled.scene().scene().nodes[1].bounds.y, 27.0);
    assert_eq!(
        compiled.scene().scene().nodes[1].content,
        wallpaper_scene::Content::Text {
            text: "x".into(),
            font_size: 40.0,
            color: wallpaper_scene::Color([0, 255, 0, 255])
        }
    );
}

#[test]
fn text_background_and_foreground_share_one_element_binding() {
    let html = "<span id='label' style='width:20px;height:20px;background-color:#fff'>x</span>";
    let compiled = compile(&package(html, ""), viewport()).unwrap();
    assert_eq!(compiled.symbols()["label"], [1, 2]);
    assert!(matches!(
        compiled.scene().scene().nodes[0].content,
        wallpaper_scene::Content::Rectangle { .. }
    ));
    assert!(matches!(
        compiled.scene().scene().nodes[1].content,
        wallpaper_scene::Content::Text { .. }
    ));
}

#[test]
fn missing_dimensions_resources_and_invalid_images_are_not_silent() {
    assert_eq!(
        compile(&package("<div style='width:1px'></div>", ""), viewport())
            .unwrap_err()
            .code,
        ErrorCode::MissingDimensions
    );
    let mut files = vec![
        ("manifest.json".into(), br#"{"formatVersion":1,"apiVersion":1,"id":"fixture","name":"fixture","entry":"index.html","permissions":[],"preferredFps":30}"#.to_vec()),
        ("index.html".into(), b"<link rel='stylesheet' href='missing.css'>".to_vec()),
    ];
    let input = wallpaper_format::Package::from_entries(files.clone()).unwrap();
    let error = compile(&input, viewport()).unwrap_err();
    assert_eq!(error.code, ErrorCode::MissingResource);
    assert_eq!(error.span.start, 29);
    files[1].1 = b"<img src='image.png' style='width:1px;height:1px'>".to_vec();
    files.push(("image.png".into(), vec![]));
    assert_eq!(
        compile(
            &wallpaper_format::Package::from_entries(files).unwrap(),
            viewport()
        )
        .unwrap_err()
        .code,
        ErrorCode::Image(wallpaper_media::Error::InvalidPng)
    );
}

#[test]
fn image_fixture_pixels_binding_and_independent_assets() {
    let compiled = compile(&fixtures::image_package().unwrap(), viewport()).unwrap();
    let decoded = wallpaper_scene::decode(&compiled.scene().encode()).unwrap();
    wallpaper_media::validate_assets(&decoded, compiled.images()).unwrap();
    assert_eq!(decoded.scene().nodes.len(), 3);
    assert_eq!(decoded.scene().resources.len(), 1);
    assert_eq!(compiled.symbols()["pattern"], [3]);
    assert_eq!(decoded.scene().nodes[2].bounds.x, 104.0);
    assert_eq!(decoded.scene().nodes[2].bounds.y, 192.0);
    assert_eq!(
        decoded.scene().nodes[2].content,
        wallpaper_scene::Content::Image { resource: 1 }
    );
    let image = &compiled.images()[&1];
    assert_eq!(
        (image.width(), image.height(), image.pixels().len()),
        (8, 8, 256)
    );
    for (x, y, rgba) in [
        (0, 0, [32, 160, 128, 255]),
        (7, 0, [230, 160, 64, 255]),
        (0, 7, [40, 64, 128, 255]),
        (7, 7, [230, 64, 96, 128]),
    ] {
        let start = (y * 8 + x) * 4;
        assert_eq!(&image.pixels()[start..start + 4], &rgba);
    }
}

#[test]
fn repeated_image_path_reuses_one_asset_and_background_precedes_image() {
    let original = fixtures::image_package().unwrap();
    let html = "<img id='first' src='checker.png' style='width:10px;height:20px;background-color:#fff'><img id='second' src='checker.png' style='width:30px;height:40px'>";
    let entries = original
        .files()
        .map(|file| {
            (
                file.path().as_str().to_owned(),
                if file.path().as_str() == "index.html" {
                    html.as_bytes().to_vec()
                } else {
                    file.bytes().to_vec()
                },
            )
        })
        .collect();
    let compiled = compile(
        &wallpaper_format::Package::from_entries(entries).unwrap(),
        viewport(),
    )
    .unwrap();
    assert_eq!(compiled.images().len(), 1);
    assert_eq!(compiled.scene().scene().resources.len(), 1);
    assert_eq!(compiled.symbols()["first"], [1, 2]);
    assert_eq!(compiled.symbols()["second"], [3]);
    assert!(matches!(
        compiled.scene().scene().nodes[0].content,
        wallpaper_scene::Content::Rectangle { .. }
    ));
    for node in &compiled.scene().scene().nodes[1..] {
        assert_eq!(
            node.content,
            wallpaper_scene::Content::Image { resource: 1 }
        );
    }
}

#[test]
fn image_errors_keep_html_location_and_reject_wrong_extension() {
    let original = fixtures::image_package().unwrap();
    for (path, bytes, expected) in [
        (
            "checker.png",
            vec![],
            ErrorCode::Image(wallpaper_media::Error::InvalidPng),
        ),
        (
            "index.html",
            b"\n<img src='missing.png' style='width:1px;height:1px'>".to_vec(),
            ErrorCode::MissingResource,
        ),
        (
            "index.html",
            b"\n<img src='style.css' style='width:1px;height:1px'>".to_vec(),
            ErrorCode::InvalidReference,
        ),
    ] {
        let entries = original
            .files()
            .map(|file| {
                (
                    file.path().as_str().to_owned(),
                    if file.path().as_str() == path {
                        bytes.clone()
                    } else {
                        file.bytes().to_vec()
                    },
                )
            })
            .collect();
        let error = compile(
            &wallpaper_format::Package::from_entries(entries).unwrap(),
            viewport(),
        )
        .unwrap_err();
        assert_eq!(error.code, expected);
        assert_eq!(error.path.as_str(), "index.html");
        assert!(error.line >= 2);
    }
}

#[test]
fn accumulated_geometry_and_viewport_are_revalidated_at_scene_boundary() {
    let html = "<div style='width:10px;height:10px;left:32768px'>\n<span style='width:1px;height:1px;left:32768px'>x</span></div>";
    let error = compile(&package(html, ""), viewport()).unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::SceneValidation(wallpaper_scene::ErrorCode::InvalidGeometry)
    );
    assert_eq!(error.line, 2);
    let invalid = Viewport {
        width: 0,
        ..viewport()
    };
    assert_eq!(
        compile(&package("", ""), invalid).unwrap_err().code,
        ErrorCode::SceneValidation(wallpaper_scene::ErrorCode::InvalidCanvas)
    );
}

#[test]
fn html_depth_node_text_and_source_budgets_are_enforced() {
    let path = path();
    let valid = "<div>".repeat(MAX_DEPTH) + &"</div>".repeat(MAX_DEPTH);
    html::parse(Source {
        path: &path,
        text: &valid,
    })
    .unwrap();
    let excessive = "<div>".repeat(MAX_DEPTH + 1) + &"</div>".repeat(MAX_DEPTH + 1);
    assert_eq!(
        html::parse(Source {
            path: &path,
            text: &excessive
        })
        .unwrap_err()
        .code,
        ErrorCode::BudgetExceeded
    );
    let nodes = "<div></div>".repeat(wallpaper_scene::MAX_NODES + 1);
    assert_eq!(
        html::parse(Source {
            path: &path,
            text: &nodes
        })
        .unwrap_err()
        .code,
        ErrorCode::BudgetExceeded
    );
    let text = format!(
        "<span>{}</span>",
        "x".repeat(wallpaper_scene::MAX_TEXT_BYTES + 1)
    );
    assert_eq!(
        html::parse(Source {
            path: &path,
            text: &text
        })
        .unwrap_err()
        .code,
        ErrorCode::BudgetExceeded
    );
    let text = " ".repeat(MAX_SOURCE_BYTES + 1);
    assert_eq!(
        html::parse(Source {
            path: &path,
            text: &text
        })
        .unwrap_err()
        .code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn css_rules_declaration_and_aggregate_work_budgets_are_enforced() {
    let path = path();
    let links = "<link rel='stylesheet' href='style.css'>".repeat(MAX_STYLESHEETS + 1);
    assert_eq!(
        html::parse(Source {
            path: &path,
            text: &links
        })
        .unwrap_err()
        .code,
        ErrorCode::BudgetExceeded
    );
    css::parse(Source {
        path: &path,
        text: &".x{width:1px}".repeat(MAX_RULES),
    })
    .unwrap();
    assert_eq!(
        css::parse(Source {
            path: &path,
            text: &".x{width:1px}".repeat(MAX_RULES + 1)
        })
        .unwrap_err()
        .code,
        ErrorCode::BudgetExceeded
    );
    assert_eq!(
        css::parse(Source {
            path: &path,
            text: &format!(".x{{{}}}", "width:1px;".repeat(33))
        })
        .unwrap_err()
        .code,
        ErrorCode::BudgetExceeded
    );
    let html = "<link rel='stylesheet' href='style.css'>".repeat(MAX_STYLESHEETS);
    let css = format!("/*{}*/", "x".repeat(100 * 1024));
    assert_eq!(
        compile(&package(&html, &css), viewport()).unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
    let html = "<link rel='stylesheet' href='style.css'>".repeat(2);
    assert_eq!(
        compile(&package(&html, &".x{width:1px}".repeat(600)), viewport())
            .unwrap_err()
            .code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn unsupported_even_unmatched_css_and_injected_controls_remain_errors() {
    let path = path();
    for text in [
        "<span>&#0;</span>",
        "<span>&#+65;</span>",
        "<span>&#x+41;</span>",
        "<span>&unknown;</span>",
        "<div style='width:&#49;px'></div>",
    ] {
        assert!(html::parse(Source { path: &path, text }).is_err());
    }
    let html = "<link rel='stylesheet' href='style.css'><div style='width:1px;height:1px'></div>";
    assert_eq!(
        compile(&package(html, ".unused {opacity:0.5}"), viewport())
            .unwrap_err()
            .code,
        ErrorCode::UnsupportedProperty
    );
    assert!(
        css::parse(Source {
            path: &path,
            text: ".x{width:1.px}"
        })
        .is_err()
    );
}

#[test]
fn deterministic_source_mutations_do_not_panic_or_skip_output_validation() {
    let original = b"<span id='label' style='width:30px;height:30px'>original</span>";
    let mut seed = 0x1983_2411u32;
    for _ in 0..512 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let mut bytes = original.to_vec();
        bytes[seed as usize % original.len()] ^= (seed >> 24) as u8;
        if let Ok(html) = std::str::from_utf8(&bytes)
            && let Ok(compiled) = compile(&package(html, ""), viewport())
        {
            assert_eq!(
                &wallpaper_scene::decode(&compiled.scene().encode()).unwrap(),
                compiled.scene()
            );
        }
    }
}
