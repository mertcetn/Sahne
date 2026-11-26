use std::{io, process::ExitCode};
use wallpaper_compiler::{
    Viewport,
    provenance::{Mode, verify_compiled, write_compiled},
};
use wallpaper_platform::package_directory::collect;
use wallpaper_scene::Color;

pub enum Action {
    Check,
    Export,
    Verify,
    CheckScript,
}

pub fn run(path: &str, mode: &str, action: Action) -> ExitCode {
    let mode = match mode {
        "flat" => Mode::Flat,
        "groups" => Mode::Groups,
        _ => {
            eprintln!("Package mode must be flat or groups");
            return ExitCode::from(2);
        }
    };
    let result = (|| {
        let package = collect(path)?;
        let viewport = Viewport {
            width: 1920,
            height: 1080,
            background: Color([12, 17, 22, 255]),
        };
        match action {
            Action::Check => {
                write_compiled(io::sink(), &package, viewport, mode)?;
                println!("Trusted package compiled and validated.");
            }
            Action::Export => write_compiled(io::stdout().lock(), &package, viewport, mode)?,
            Action::Verify => {
                verify_compiled(io::stdin().lock(), &package, viewport, mode)?;
                println!("Compiled record matches the trusted package.");
            }
            Action::CheckScript => {
                let compiled = wallpaper_compiler::compile(&package, viewport)?;
                wallpaper_media::validate_assets(compiled.scene(), compiled.images())?;
                let script = compiled.script().ok_or("Package must declare a script")?;
                let source = package.require_text(script, ".js", "script")?;
                let targets = compiled.script_targets();
                let count = targets.len();
                wallpaper_protocol::write_frame(
                    &mut io::sink(),
                    &wallpaper_protocol::Message {
                        session: 1,
                        sequence: 2,
                        base_revision: 0,
                        body: wallpaper_protocol::Body::SceneProgram {
                            targets,
                            source: source.to_owned(),
                            initial: Vec::new(),
                        },
                    },
                )?;
                println!(
                    "Trusted script package validated: {count} bindings, {} source bytes.",
                    source.len()
                );
            }
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Trusted package failed: {error}");
            ExitCode::from(1)
        }
    }
}
