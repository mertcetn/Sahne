//! Observe lifecycle messages on an owned Windows preview window.
use std::{cell::RefCell, collections::BTreeMap, io, panic, rc::Rc, sync::Arc};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::RemoteDesktop::{
        NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
    },
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND, WM_NCDESTROY, WM_POWERBROADCAST,
            WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
        },
    },
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    SessionLocked(bool),
    SystemSuspended(bool),
}

type Handler = Rc<dyn Fn(Event)>;
thread_local! {
    static HANDLERS: RefCell<BTreeMap<usize, Handler>> = const { RefCell::new(BTreeMap::new()) };
}

const SUBCLASS_ID: usize = 1;

pub struct Subscription {
    hwnd: HWND,
    _window: Arc<Window>,
}

impl Subscription {
    /// Register on the thread that created the window.
    pub fn new(window: Arc<Window>, handler: impl Fn(Event) + 'static) -> io::Result<Self> {
        let handle = window.window_handle().map_err(io::Error::other)?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err(io::Error::other("Expected a Windows window handle"));
        };
        let hwnd = handle.hwnd.get() as HWND;
        HANDLERS.with(|handlers| {
            let mut handlers = handlers.borrow_mut();
            let std::collections::btree_map::Entry::Vacant(entry) = handlers.entry(hwnd as usize)
            else {
                return Err(io::Error::other(
                    "Window already has a lifecycle subscription",
                ));
            };
            entry.insert(Rc::new(handler));
            Ok(())
        })?;
        // SAFETY: The owned HWND is alive, and this runs on its window thread.
        if unsafe { SetWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID, 0) } == 0 {
            remove_handler(hwnd);
            return Err(io::Error::other(
                "Could not install window lifecycle callback",
            ));
        }
        // SAFETY: HWND is the live subscribed window. Register only for its session.
        if unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) } == 0 {
            let error = io::Error::last_os_error();
            unsubscribe(hwnd);
            return Err(error);
        }
        Ok(Self {
            hwnd,
            _window: window,
        })
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        unsubscribe(self.hwnd);
    }
}

fn remove_handler(hwnd: HWND) -> bool {
    HANDLERS.with(|handlers| handlers.borrow_mut().remove(&(hwnd as usize)).is_some())
}

fn unsubscribe(hwnd: HWND) {
    if remove_handler(hwnd) {
        // SAFETY: Only the registered window thread calls this for the owned HWND.
        unsafe {
            WTSUnRegisterSessionNotification(hwnd);
            RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID);
        }
    }
}

fn event(message: u32, code: WPARAM) -> Option<Event> {
    match (message, u32::try_from(code).ok()?) {
        (WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK) => Some(Event::SessionLocked(true)),
        (WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK) => Some(Event::SessionLocked(false)),
        (WM_POWERBROADCAST, PBT_APMSUSPEND) => Some(Event::SystemSuspended(true)),
        (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC) => Some(Event::SystemSuspended(false)),
        _ => None,
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    let handled = if message == WM_NCDESTROY {
        unsubscribe(hwnd);
        false
    } else if let Some(event) = event(message, wparam) {
        let handler = HANDLERS.with(|handlers| handlers.borrow().get(&(hwnd as usize)).cloned());
        handler.is_some_and(|handler| {
            panic::catch_unwind(panic::AssertUnwindSafe(|| handler(event))).is_ok()
        })
    } else {
        false
    };
    // SAFETY: Forward the original message within the active subclass call.
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if handled && message == WM_POWERBROADCAST {
        1
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_supported_power_and_session_messages() {
        let decode = |message, state: u32| event(message, state as WPARAM);
        assert_eq!(
            decode(WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK),
            Some(Event::SessionLocked(true))
        );
        assert_eq!(
            decode(WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK),
            Some(Event::SessionLocked(false))
        );
        assert_eq!(
            decode(WM_POWERBROADCAST, PBT_APMSUSPEND),
            Some(Event::SystemSuspended(true))
        );
        assert_eq!(
            decode(WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC),
            Some(Event::SystemSuspended(false))
        );
        assert_eq!(decode(WM_POWERBROADCAST, 0), None);
        assert_eq!(decode(WM_NCDESTROY, WTS_SESSION_LOCK), None);
    }
}
