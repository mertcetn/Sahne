//! Windows AppContainer spike. Trusted native FFI; never enables unsandboxed fallback.
#![deny(unsafe_op_in_unsafe_fn)]
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
