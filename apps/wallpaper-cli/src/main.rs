use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            println!(
                "Sahne development CLI\nUsage: wallpaper-cli [--help | --version]\nWallpaper execution and package validation are not implemented yet."
            );
            ExitCode::SUCCESS
        }
        [arg] if arg == "--help" || arg == "-h" => {
            println!(
                "Usage: wallpaper-cli [--help | --version]\nDevelopment scaffold only; no wallpaper execution or package validation."
            );
            ExitCode::SUCCESS
        }
        [arg] if arg == "--version" || arg == "-V" => {
            println!("wallpaper-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("Unsupported arguments. Run wallpaper-cli --help.");
            ExitCode::from(2)
        }
    }
}
