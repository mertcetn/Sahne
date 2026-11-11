#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    windows::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The trusted AppContainer probe is Windows-only.");
    std::process::exit(2);
}
