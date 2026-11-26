//! Trusted flat package preparation for the one-shot worker bridge.
use super::*;
use wallpaper_protocol::Body;
use wallpaper_scene::ValidatedScene;

pub struct Prepared {
    pub scene: Arc<ValidatedScene>,
    pub program: Body,
    pub lifecycle: Lifecycle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    OneShot,
    Check,
    Live,
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
    wallpaper_protocol::validate_scene_script_source(source)?;
    let targets = compiled.script_targets();
    let initial = wallpaper_protocol::initial_scene_edits(compiled.scene(), &targets)?;
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
        lifecycle: Lifecycle::OneShot,
    })
}

pub fn prepare_isolated(folder: &str) -> Result<Prepared> {
    let (output, program) = super::package_intake::load_script(folder)?;
    let Body::SceneProgram {
        targets, initial, ..
    } = &program
    else {
        return Err("Expected isolated script program".into());
    };
    if !output.images.is_empty()
        || wallpaper_protocol::initial_scene_edits(&output.scene, targets)? != *initial
    {
        return Err("Isolated script bindings do not match the validated scene".into());
    }
    Ok(Prepared {
        scene: Arc::new(output.scene),
        program,
        lifecycle: Lifecycle::OneShot,
    })
}
