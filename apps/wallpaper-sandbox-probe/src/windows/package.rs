//! Original fixed sources only; no filesystem input or script execution.
use super::Result;
use wallpaper_format::{ErrorCode, Package};

pub const RESULT_BIT: u32 = 1 << 18;
pub const COMPILER_RESULT_BIT: u32 = 1 << 19;
const MANIFEST: &str = r#"{"formatVersion":1,"apiVersion":1,"id":"sahne.fixture","name":"Original sandbox fixture","entry":"index.html","permissions":[],"preferredFps":30}"#;
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
