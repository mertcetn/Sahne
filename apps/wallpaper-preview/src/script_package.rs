//! Trusted flat package preparation for the one-shot worker bridge.
use super::*;
use wallpaper_protocol::Body;
use wallpaper_scene::{Content, ValidatedScene, transaction::Edit};

pub struct Prepared {
    pub scene: Arc<ValidatedScene>,
    pub program: Body,
}

pub fn prepare(folder: &str) -> Result<Prepared> {
    let package = wallpaper_platform::package_directory::collect(folder)?;
    let compiled = wallpaper_compiler::compile(
        &package,
        wallpaper_compiler::Viewport {
            width: 1920,
            height: 1080,
            background: wallpaper_scene::Color([0; 4]),
        },
    )?;
    if !compiled.scene().scene().resources.is_empty() {
        return Err("Trusted script preview supports text and rectangles only".into());
    }
    let source = package.require_text(
        compiled.script().ok_or("Package must declare a script")?,
        ".js",
        "script",
    )?;
    let targets = compiled.script_targets();
    let mut initial = Vec::new();
    for &node in targets.values() {
        let content = &compiled
            .scene()
            .scene()
            .nodes
            .iter()
            .find(|item| item.id == node)
            .ok_or("Missing compiled script target")?
            .content;
        let color = match content {
            Content::Text { text, color, .. } => {
                initial.push(Edit::Text {
                    node,
                    value: text.clone(),
                });
                *color
            }
            Content::Rectangle { color } => *color,
            _ => return Err("Unsupported script binding content".into()),
        };
        initial.push(Edit::Color { node, value: color });
    }
    let program = wallpaper_protocol::Message {
        session: 1,
        sequence: 2,
        base_revision: 0,
        body: Body::SceneProgram {
            targets,
            source: source.into(),
            initial,
        },
    };
    wallpaper_protocol::write_frame(&mut std::io::sink(), &program)?;
    Ok(Prepared {
        scene: Arc::new(compiled.into_scene()),
        program: program.body,
    })
}
