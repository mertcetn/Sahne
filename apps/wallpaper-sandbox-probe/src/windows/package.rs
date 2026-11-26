//! Package preparation and fixed compiler fixtures. Sources are supplied in memory.
use super::Result;
use wallpaper_format::{ErrorCode, Package};
use wallpaper_protocol::{Body, Message, initial_scene_edits, write_frame};

pub fn script_program(
    package: &Package,
    compiled: &wallpaper_compiler::Compiled,
    session: u64,
) -> Result<Message> {
    if !compiled.images().is_empty() {
        return Err("Script package must use text and rectangles only".into());
    }
    let source = package.require_text(
        compiled.script().ok_or("Expected package script")?,
        ".js",
        "script",
    )?;
    wallpaper_protocol::validate_scene_script_source(source)?;
    let targets = compiled.script_targets();
    let initial = initial_scene_edits(compiled.scene(), &targets)?;
    let program = Message {
        session,
        sequence: 3,
        base_revision: 0,
        body: Body::SceneProgram {
            targets,
            source: source.into(),
            initial,
        },
    };
    write_frame(&mut std::io::sink(), &program)?;
    Ok(program)
}

pub const RESULT_BIT: u32 = 1 << 18;
pub const COMPILER_RESULT_BIT: u32 = 1 << 19;
pub const IMAGE_RESULT_BIT: u32 = 1 << 20;
pub const HIERARCHY_RESULT_BIT: u32 = 1 << 21;
pub const GROUP_COMPILER_RESULT_BIT: u32 = 1 << 22;
const MANIFEST: &str = r#"{"formatVersion":1,"apiVersion":1,"id":"sahne.fixture","name":"Original sandbox fixture","entry":"index.html","permissions":[],"preferredFps":30}"#;

pub fn run_group_compiler(case: &str) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Group compiler fixture requires AppContainer".into());
    }
    let original = wallpaper_compiler::fixtures::groups_package()?;
    let viewport = wallpaper_compiler::Viewport {
        width: 1920,
        height: 1080,
        background: wallpaper_scene::Color([12, 17, 22, 255]),
    };
    match case {
        "groups-basic" => {
            let compiled = wallpaper_compiler::compile_groups(&original, viewport)?;
            let decoded = wallpaper_scene::hierarchy::decode(&compiled.scene().encode())?;
            wallpaper_media::validate_hierarchy_assets(&decoded, compiled.images())?;
            let image = compiled.images().get(&1).ok_or("Missing group image")?;
            if &decoded != compiled.scene()
                || decoded.scene().layers.len() != 10
                || decoded.commands().len() != 15
                || compiled.symbols().get("blue") != Some(&5)
                || decoded.resolved()[4].bounds.x != 190.0
                || decoded.resolved()[4].bounds.y != 200.0
                || image.pixels().len() != 256
                || image.pixels()[252..] != [230, 64, 96, 128]
            {
                return Err("Group compiler fixed result mismatch".into());
            }
        }
        "groups-invalid" => {
            use wallpaper_compiler::ErrorCode as Code;
            for (css, expected) in [
                (".group{transform:rotate(10deg)}", Code::UnsupportedValue),
                (".group{opacity:1.1}", Code::InvalidNumber),
                (".group{overflow:hidden}", Code::UnsupportedValue),
                (
                    ".group{width:10px;height:10px;transform:scale(2)}",
                    Code::MissingTransformOrigin,
                ),
            ] {
                let entries = original
                    .files()
                    .map(|file| {
                        (
                            file.path().as_str().to_owned(),
                            if file.path().as_str() == "style.css" {
                                css.as_bytes().to_vec()
                            } else {
                                file.bytes().to_vec()
                            },
                        )
                    })
                    .collect();
                let error =
                    wallpaper_compiler::compile_groups(&Package::from_entries(entries)?, viewport)
                        .unwrap_err();
                if error.code != expected {
                    return Err("Group compiler fixed rejection mismatch".into());
                }
            }
        }
        _ => return Err("Unknown fixed group compiler fixture".into()),
    }
    Ok(())
}

pub fn run_hierarchy(case: &str) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Hierarchy fixture requires AppContainer".into());
    }
    use wallpaper_scene::{
        ErrorCode as Code,
        hierarchy::{self, LayerContent},
    };
    let original = hierarchy::fixtures::original();
    match case {
        "hierarchy-basic" => {
            let scene = original.validate()?;
            let decoded = hierarchy::decode(&scene.encode())?;
            if decoded != scene
                || decoded.commands().len() != 5
                || decoded.resolved()[2].bounds.x != 190.0
                || decoded.resolved()[2].bounds.y != 200.0
            {
                return Err("Hierarchy fixed scene/command mismatch".into());
            }
        }
        "hierarchy-invalid" => {
            let mut parent = original.clone();
            parent.layers[1].parent = 4;
            let mut opacity = original.clone();
            if let LayerContent::Group { opacity, .. } = &mut opacity.layers[0].content {
                *opacity = 1.01;
            }
            let mut transform = original.clone();
            if let LayerContent::Group { transform, .. } = &mut transform.layers[0].content {
                transform.scale[0] = f32::INFINITY;
            }
            for (data, expected) in [
                (parent, Code::InvalidHierarchy),
                (opacity, Code::InvalidEffect),
                (transform, Code::NonFinite),
            ] {
                if data.validate().unwrap_err().code != expected {
                    return Err("Hierarchy fixed rejection mismatch".into());
                }
            }
            let bytes = original.validate()?.encode();
            for end in 0..bytes.len() {
                if hierarchy::decode(&bytes[..end]).is_ok() {
                    return Err("Truncated hierarchy accepted".into());
                }
            }
            let mut flag = bytes;
            flag[75] = 2;
            if hierarchy::decode(&flag).unwrap_err().code != Code::InvalidEncoding {
                return Err("Unknown hierarchy clip flag accepted".into());
            }
        }
        _ => return Err("Unknown fixed hierarchy fixture".into()),
    }
    Ok(())
}

pub fn run_image(case: &str) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Image fixture requires AppContainer".into());
    }
    let original = wallpaper_compiler::fixtures::image_package()?;
    let viewport = wallpaper_compiler::Viewport {
        width: 1920,
        height: 1080,
        background: wallpaper_scene::Color([12, 17, 22, 255]),
    };
    match case {
        "image-basic" => {
            let compiled = wallpaper_compiler::compile(&original, viewport)?;
            let decoded = wallpaper_scene::decode(&compiled.scene().encode())?;
            wallpaper_media::validate_assets(&decoded, compiled.images())?;
            let image = compiled.images().get(&1).ok_or("Missing fixed image")?;
            if decoded.scene().nodes.len() != 3
                || decoded.scene().resources.len() != 1
                || image.width() != 8
                || image.height() != 8
                || image.pixels().len() != 256
                || image.pixels()[..4] != [32, 160, 128, 255]
                || image.pixels()[252..] != [230, 64, 96, 128]
            {
                return Err("Image fixture changed pixels or metadata".into());
            }
        }
        "image-invalid" => {
            use wallpaper_media::Error as Code;
            let path = wallpaper_format::RelativePath::parse("checker.png")?;
            let bytes = original.file(&path).ok_or("Missing image source")?.bytes();
            let mut crc = bytes.to_vec();
            crc[29] ^= 1;
            for (data, budget, expected) in [
                (vec![], 256, Code::InvalidPng),
                (bytes[..bytes.len() - 1].to_vec(), 256, Code::InvalidPng),
                (crc, 256, Code::Checksum),
                (bytes.to_vec(), 255, Code::BudgetExceeded),
            ] {
                if wallpaper_media::decode_png(&data, budget).unwrap_err() != expected {
                    return Err("Image fixture rejection mismatch".into());
                }
            }
            let entries = original
                .files()
                .map(|file| {
                    (
                        file.path().as_str().to_owned(),
                        if file.path() == &path {
                            vec![]
                        } else {
                            file.bytes().to_vec()
                        },
                    )
                })
                .collect();
            let package = Package::from_entries(entries)?;
            let error = wallpaper_compiler::compile(&package, viewport).unwrap_err();
            if error.code != wallpaper_compiler::ErrorCode::Image(Code::InvalidPng)
                || error.path.as_str() != "index.html"
                || error.line != 4
            {
                return Err("Image source diagnostic mismatch".into());
            }
        }
        _ => return Err("Unknown fixed image fixture".into()),
    }
    Ok(())
}
fn files(manifest: String) -> Vec<(String, Vec<u8>)> {
    vec![
        ("manifest.json".into(), manifest.into_bytes()),
        ("index.html".into(), b"<div id=\"original\"></div>".to_vec()),
    ]
}
pub fn run(case: &str) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Package fixture requires AppContainer".into());
    }
    match case {
        "package-basic" => {
            let package = Package::from_entries(files(MANIFEST.into()))?;
            if package.manifest().id() != "sahne.fixture"
                || package.manifest().preferred_fps() != 30
                || package.require_text(package.manifest().entry(), ".html", "entry")?
                    != "<div id=\"original\"></div>"
                || package.files().len() != 2
            {
                return Err("Package fixture changed validated metadata/source".into());
            }
        }
        "package-invalid" => {
            for (from, to, expected) in [
                (
                    "\"permissions\":[]",
                    "\"permissions\":[\"network\"]",
                    ErrorCode::ManifestSchema,
                ),
                (
                    "\"formatVersion\":1",
                    "\"formatVersion\":2",
                    ErrorCode::UnsupportedVersion,
                ),
                ("index.html", "../outside.html", ErrorCode::InvalidPath),
                (
                    "\"preferredFps\":30",
                    "\"preferredFps\":61",
                    ErrorCode::InvalidFps,
                ),
            ] {
                let error = Package::from_entries(files(MANIFEST.replace(from, to))).unwrap_err();
                if error.code != expected {
                    return Err("Package fixture rejection mismatch".into());
                }
            }
            let mut collision = files(MANIFEST.into());
            collision.push(("INDEX.html".into(), vec![]));
            if Package::from_entries(collision).unwrap_err().code != ErrorCode::CaseCollision {
                return Err("Package fixture case collision accepted".into());
            }
        }
        _ => return Err("Unknown fixed package fixture".into()),
    }
    Ok(())
}

pub fn run_compiler(case: &str) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Compiler fixture requires AppContainer".into());
    }
    let viewport = wallpaper_compiler::Viewport {
        width: 1920,
        height: 1080,
        background: wallpaper_scene::Color([12, 17, 22, 255]),
    };
    let original = wallpaper_compiler::fixtures::static_package()?;
    match case {
        "compiler-basic" => {
            let compiled = wallpaper_compiler::compile(&original, viewport)?;
            let decoded = wallpaper_scene::decode(&compiled.scene().encode())?;
            if &decoded != compiled.scene()
                || decoded.scene().nodes.len() != 3
                || decoded.scene().nodes[1].bounds.x != 104.0
                || decoded.scene().nodes[1].bounds.y != 104.0
                || compiled.symbols().get("title").map(Vec::as_slice) != Some(&[2][..])
            {
                return Err("Compiler fixed scene mismatch".into());
            }
        }
        "compiler-invalid" => {
            use wallpaper_compiler::ErrorCode as Code;
            for (path, source, expected) in [
                (
                    "style.css",
                    ".panel{display:flex}",
                    Code::UnsupportedProperty,
                ),
                ("style.css", ".panel{width:NaNpx}", Code::InvalidNumber),
                ("style.css", ".panel{width:100%}", Code::InvalidNumber),
                (
                    "style.css",
                    ".panel{opacity:0.5}",
                    Code::UnsupportedProperty,
                ),
                (
                    "index.html",
                    "<div id='same'></div><span id='same'></span>",
                    Code::DuplicateId,
                ),
                (
                    "index.html",
                    "<div><span></div></span>",
                    Code::MismatchedTag,
                ),
                (
                    "index.html",
                    "<img src='https://example.test/image.png'>",
                    Code::InvalidReference,
                ),
                (
                    "index.html",
                    "<script>throw new Error('never execute')</script>",
                    Code::UnsupportedElement,
                ),
            ] {
                let entries = original
                    .files()
                    .map(|file| {
                        (
                            file.path().as_str().to_owned(),
                            if file.path().as_str() == path {
                                source.as_bytes().to_vec()
                            } else {
                                file.bytes().to_vec()
                            },
                        )
                    })
                    .collect();
                let package = Package::from_entries(entries)?;
                if wallpaper_compiler::compile(&package, viewport)
                    .unwrap_err()
                    .code
                    != expected
                {
                    return Err("Compiler fixed rejection mismatch".into());
                }
            }
        }
        _ => return Err("Unknown fixed compiler fixture".into()),
    }
    Ok(())
}
