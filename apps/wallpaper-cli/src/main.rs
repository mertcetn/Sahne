use std::{ffi::OsString, process::ExitCode};

#[cfg(windows)]
mod local_package;

const LOCAL_PACKAGE_HELP: &str = "\
--check-trusted-package <absolute-folder> <flat|groups> validates and compiles a local native package on Windows.
--export-trusted-package <absolute-folder> <flat|groups> also writes its binary version 2 provenance record to stdout.
--verify-trusted-package <absolute-folder> <flat|groups> checks a binary record from stdin against that folder.
--check-trusted-script-package <absolute-folder> validates a flat package and checks its script initialization frame without executing JavaScript.
Use local packages you trust. These developer commands run in the CLI process.";

fn main() -> ExitCode {
    let args: Result<Vec<String>, OsString> = std::env::args_os()
        .skip(1)
        .map(OsString::into_string)
        .collect();
    let Ok(args) = args else {
        eprintln!("Arguments must be valid UTF-8");
        return ExitCode::from(2);
    };
    match args.as_slice() {
        [] => {
            println!("Sahne development CLI\nRun wallpaper-cli --help for available commands.");
            ExitCode::SUCCESS
        }
        [arg] if arg == "--help" || arg == "-h" => {
            println!(
                "Usage: wallpaper-cli [--help | --version | --scene-check | --hierarchy-check | --compile-check | --compile-image-check | --compile-groups-check | --compiled-provenance-fixture [static|image|groups|animated-shapes]]\n--scene-check validates and roundtrips a fixed built-in flat scene.\n--hierarchy-check validates and roundtrips an original version-2 group fixture; no group rendering.\n--compile-check compiles original built-in HTML/CSS and independently validates the scene.\n--compile-image-check also decodes an original fixed PNG and validates its pixel assets against the scene.\n--compile-groups-check compiles original HTML/CSS/PNG into version-2 groups and independently checks scene/assets.\n--compiled-provenance-fixture writes a binary provenance record to stdout. An omitted name selects the legacy groups record.\n--verify-compiled-provenance-fixture [static|image|groups|animated-shapes] checks binary stdin against the selected embedded fixture.\nThese checks perform no rendering or script execution."
            );
            println!("{LOCAL_PACKAGE_HELP}");
            ExitCode::SUCCESS
        }
        [arg] if arg == "--version" || arg == "-V" => {
            println!("wallpaper-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [arg] if arg == "--scene-check" => match check_scene() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Scene self-check failed: {error}");
                ExitCode::from(1)
            }
        },
        [arg] if arg == "--hierarchy-check" => match check_hierarchy() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Hierarchy self-check failed: {error}");
                ExitCode::from(1)
            }
        },
        [arg] if arg == "--compile-groups-check" => match check_group_compiler() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Group compiler self-check failed: {error}");
                ExitCode::from(1)
            }
        },
        [arg] if arg == "--compile-check" || arg == "--compile-image-check" => {
            match check_compiler(arg == "--compile-image-check") {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Compiler self-check failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        [arg] if arg == "--verify-compiled-provenance-fixture" => provenance_fixture(None, true),
        [arg, name] if arg == "--verify-compiled-provenance-fixture" => {
            provenance_fixture(Some(name), true)
        }
        [arg] if arg == "--compiled-provenance-fixture" => provenance_fixture(None, false),
        [arg, name] if arg == "--compiled-provenance-fixture" => {
            provenance_fixture(Some(name), false)
        }
        #[cfg(windows)]
        [arg, path] if arg == "--check-trusted-script-package" => {
            local_package::run(path, "flat", local_package::Action::CheckScript)
        }
        #[cfg(windows)]
        [arg, path, mode]
            if matches!(
                arg.as_str(),
                "--check-trusted-package" | "--export-trusted-package" | "--verify-trusted-package"
            ) =>
        {
            use local_package::Action;
            let action = match arg.as_str() {
                "--check-trusted-package" => Action::Check,
                "--export-trusted-package" => Action::Export,
                _ => Action::Verify,
            };
            local_package::run(path, mode, action)
        }
        _ => {
            eprintln!("Unsupported arguments. Run wallpaper-cli --help.");
            ExitCode::from(2)
        }
    }
}

fn check_group_compiler() -> Result<(), Box<dyn std::error::Error>> {
    let package = wallpaper_compiler::fixtures::groups_package()?;
    let compiled = wallpaper_compiler::compile_groups(
        &package,
        wallpaper_compiler::Viewport {
            width: 1920,
            height: 1080,
            background: wallpaper_scene::Color([12, 17, 22, 255]),
        },
    )?;
    let bytes = compiled.scene().encode();
    let decoded = wallpaper_scene::hierarchy::decode(&bytes)?;
    wallpaper_media::validate_hierarchy_assets(&decoded, compiled.images())?;
    if &decoded != compiled.scene()
        || decoded.encode() != bytes
        || decoded.scene().layers.len() != 10
        || decoded.commands().len() != 15
        || compiled.symbols().get("blue") != Some(&5)
        || decoded.resolved()[4].bounds.x != 190.0
        || decoded.resolved()[4].bounds.y != 200.0
    {
        return Err(std::io::Error::other(
            "Compiled group scene changed during independent validation",
        )
        .into());
    }
    println!(
        "Group compiler self-check passed: version={}, layers={}, commands={}, images={}, encoded bytes={}. Original embedded sources only; no group rendering or script execution.",
        decoded.scene().version,
        decoded.scene().layers.len(),
        decoded.commands().len(),
        compiled.images().len(),
        bytes.len()
    );
    Ok(())
}

fn check_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    use wallpaper_scene::hierarchy;
    let scene = hierarchy::fixtures::original().validate()?;
    let bytes = scene.encode();
    let decoded = hierarchy::decode(&bytes)?;
    if decoded != scene
        || decoded.encode() != bytes
        || decoded.commands().len() != 5
        || decoded.resolved()[2].bounds.x != 190.0
        || decoded.resolved()[2].bounds.y != 200.0
    {
        return Err(
            std::io::Error::other("Hierarchy roundtrip or derived commands changed").into(),
        );
    }
    println!(
        "Hierarchy self-check passed: version={}, layers={}, commands={}, encoded bytes={}. Fixed typed data only; no source parsing, rendering or script execution.",
        decoded.scene().version,
        decoded.scene().layers.len(),
        decoded.commands().len(),
        bytes.len()
    );
    Ok(())
}

fn check_compiler(image: bool) -> Result<(), Box<dyn std::error::Error>> {
    let package = if image {
        wallpaper_compiler::fixtures::image_package()?
    } else {
        wallpaper_compiler::fixtures::static_package()?
    };
    let compiled = wallpaper_compiler::compile(
        &package,
        wallpaper_compiler::Viewport {
            width: 1920,
            height: 1080,
            background: wallpaper_scene::Color([12, 17, 22, 255]),
        },
    )?;
    let bytes = compiled.scene().encode();
    let decoded = wallpaper_scene::decode(&bytes)?;
    wallpaper_media::validate_assets(&decoded, compiled.images())?;
    if &decoded != compiled.scene() || decoded.encode() != bytes {
        return Err(
            std::io::Error::other("Compiled scene changed during independent decode").into(),
        );
    }
    println!(
        "Compiler self-check passed: nodes={}, named elements={}, encoded bytes={}, images={}. Original embedded sources only; no rendering or script execution.",
        decoded.scene().nodes.len(),
        compiled.symbols().len(),
        bytes.len(),
        compiled.images().len()
    );
    Ok(())
}

fn check_scene() -> Result<(), Box<dyn std::error::Error>> {
    use wallpaper_scene::{Color, Content, ImageResource, Node, Rect, SCENE_VERSION, Scene};
    // Original fixed data only: resource dimensions are metadata, not an image decoder.
    let scene = Scene {
        version: SCENE_VERSION,
        width: 1920,
        height: 1080,
        background: Color([16, 22, 28, 255]),
        resources: vec![ImageResource {
            id: 1,
            width: 2,
            height: 2,
        }],
        nodes: vec![
            Node {
                id: 1,
                bounds: Rect {
                    x: 80.0,
                    y: 80.0,
                    width: 640.0,
                    height: 240.0,
                },
                content: Content::Rectangle {
                    color: Color([40, 80, 120, 255]),
                },
            },
            Node {
                id: 2,
                bounds: Rect {
                    x: 100.0,
                    y: 110.0,
                    width: 560.0,
                    height: 80.0,
                },
                content: Content::Text {
                    text: "Sahne scene foundation".into(),
                    font_size: 32.0,
                    color: Color([255; 4]),
                },
            },
            Node {
                id: 3,
                bounds: Rect {
                    x: 760.0,
                    y: 80.0,
                    width: 160.0,
                    height: 160.0,
                },
                content: Content::Image { resource: 1 },
            },
        ],
    }
    .validate()?;
    let bytes = scene.encode();
    let decoded = wallpaper_scene::decode(&bytes)?;
    if decoded != scene || decoded.encode() != bytes {
        return Err(std::io::Error::other("Scene roundtrip changed validated data").into());
    }
    println!(
        "Scene self-check passed: version={}, nodes={}, resources={}, encoded bytes={}. Fixed data only; no rendering or external input.",
        decoded.scene().version,
        decoded.scene().nodes.len(),
        decoded.scene().resources.len(),
        bytes.len()
    );
    Ok(())
}

fn provenance_fixture(name: Option<&str>, verify: bool) -> ExitCode {
    if name.is_some_and(|name| !matches!(name, "static" | "image" | "groups" | "animated-shapes")) {
        eprintln!("Unknown compiled provenance fixture");
        return ExitCode::from(2);
    }
    use wallpaper_compiler::provenance;
    let result = if verify {
        provenance::verify_fixture(std::io::stdin().lock(), name)
    } else {
        provenance::write_fixture(std::io::stdout().lock(), name)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Compiled provenance fixture failed: {error}");
            ExitCode::from(1)
        }
    }
}
