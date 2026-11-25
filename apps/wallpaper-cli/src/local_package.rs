use std::{io, process::ExitCode};
use wallpaper_compiler::{
    Viewport,
    provenance::{Mode, write_compiled},
};
use wallpaper_platform::package_directory::collect;
use wallpaper_scene::Color;

pub fn run(path: &str, mode: &str, export: bool) -> ExitCode {
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
        if export {
            write_compiled(io::stdout().lock(), &package, viewport, mode)?;
        } else {
            write_compiled(io::sink(), &package, viewport, mode)?;
            println!("Trusted package compiled and validated.");
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
