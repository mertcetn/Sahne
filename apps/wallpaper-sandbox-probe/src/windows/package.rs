//! Original fixed sources only; no filesystem input or script execution.
use super::Result;
use wallpaper_format::{ErrorCode, Package};

pub const RESULT_BIT: u32 = 1 << 18;
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
