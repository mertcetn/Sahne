use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            println!(
                "Sahne development CLI\nUsage: wallpaper-cli [--help | --version | --scene-check | --compile-check | --compile-image-check]\nExternal package loading and wallpaper execution are not implemented."
            );
            ExitCode::SUCCESS
        }
        [arg] if arg == "--help" || arg == "-h" => {
            println!(
                "Usage: wallpaper-cli [--help | --version | --scene-check | --compile-check | --compile-image-check]\n--scene-check validates and roundtrips a fixed built-in scene.\n--compile-check compiles original built-in HTML/CSS and independently validates the scene.\n--compile-image-check also decodes an original fixed PNG and validates its pixel assets against the scene.\nNo external files, rendering or script execution in these checks."
            );
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
        [arg] if arg == "--compile-check" || arg == "--compile-image-check" => {
            match check_compiler(arg == "--compile-image-check") {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Compiler self-check failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!("Unsupported arguments. Run wallpaper-cli --help.");
            ExitCode::from(2)
        }
    }
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
