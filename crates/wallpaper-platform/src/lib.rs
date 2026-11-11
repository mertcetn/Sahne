//! Narrow Windows desktop FFI boundary; never used by untrusted workers.
#![deny(unsafe_op_in_unsafe_fn)]
#[cfg(windows)]
mod desktop {
    use std::{io, ptr};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::*,
    };
    use winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        platform::windows::WindowAttributesExtWindows,
        window::WindowAttributes,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    // SAFETY: EnumWindows invokes this synchronously with a valid pointer to our local HWND.
    unsafe extern "system" fn find_worker(window: HWND, output: LPARAM) -> i32 {
        let defview = wide("SHELLDLL_DefView");
        let worker = wide("WorkerW");
        // SAFETY: Class strings are terminated and alive for the calls. Windows validates HWNDs.
        unsafe {
            if !FindWindowExW(window, ptr::null_mut(), defview.as_ptr(), ptr::null()).is_null() {
                let candidate =
                    FindWindowExW(ptr::null_mut(), window, worker.as_ptr(), ptr::null());
                if !candidate.is_null() {
                    *(output as *mut HWND) = candidate;
                    return 0;
                }
            }
        }
        1
    }
    pub struct Desktop {
        parent: HWND,
    }
    impl Desktop {
        pub fn connect() -> io::Result<Self> {
            let progman_class = wide("Progman");
            let worker_class = wide("WorkerW");
            // SAFETY: String and result pointers are valid during synchronous bounded calls.
            unsafe {
                let progman = FindWindowW(progman_class.as_ptr(), ptr::null());
                if progman.is_null() {
                    return Err(io::Error::other("Explorer Progman was not found"));
                }
                if SendMessageTimeoutW(
                    progman,
                    0x052c,
                    0xD,
                    1,
                    SMTO_ABORTIFHUNG,
                    1000,
                    ptr::null_mut(),
                ) == 0
                {
                    return Err(io::Error::other("Explorer desktop request timed out"));
                }
                let mut parent: HWND = ptr::null_mut();
                EnumWindows(Some(find_worker), &mut parent as *mut HWND as LPARAM);
                if parent.is_null() {
                    parent =
                        FindWindowExW(progman, ptr::null_mut(), worker_class.as_ptr(), ptr::null());
                }
                if parent.is_null() || IsWindow(parent) == 0 {
                    return Err(io::Error::other(
                        "No compatible WorkerW surface; ordinary-window fallback is disabled",
                    ));
                }
                Ok(Self { parent })
            }
        }
        pub fn alive(&self) -> bool {
            // SAFETY: IsWindow checks the opaque OS handle; no Rust memory is dereferenced.
            unsafe { IsWindow(self.parent) != 0 }
        }
        pub fn attributes(&self, attrs: WindowAttributes) -> io::Result<WindowAttributes> {
            let mut rect = RECT::default();
            // SAFETY: rect is initialized and writable; parent is validated by the OS.
            if unsafe { GetClientRect(self.parent, &mut rect) } == 0 {
                return Err(io::Error::last_os_error());
            }
            if rect.right <= 0 || rect.bottom <= 0 {
                return Err(io::Error::other("Desktop has an empty client area"));
            }
            // SAFETY: winit receives the live Explorer-owned HWND; App monitors its lifetime.
            Ok(unsafe {
                attrs.with_parent_window(Some(winit::raw_window_handle::RawWindowHandle::Win32(
                    winit::raw_window_handle::Win32WindowHandle::new(
                        std::num::NonZeroIsize::new(self.parent as isize)
                            .expect("validated non-null HWND"),
                    ),
                )))
            }
            .with_decorations(false)
            .with_skip_taskbar(true)
            .with_position(PhysicalPosition::new(0, 0))
            .with_inner_size(PhysicalSize::new(rect.right as u32, rect.bottom as u32)))
        }
    }
}
#[cfg(windows)]
pub use desktop::Desktop;
