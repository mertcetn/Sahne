//! Observe lifecycle messages on an owned Windows preview window.
use std::{cell::RefCell, collections::BTreeMap, io, panic, rc::Rc, sync::Arc};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::Diagnostics::Debug::ReadProcessMemory,
    System::Power::{
        HPOWERNOTIFY, RegisterPowerSettingNotification, UnregisterPowerSettingNotification,
    },
    System::RemoteDesktop::{
        NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
    },
    System::{SystemServices::GUID_SESSION_DISPLAY_STATUS, Threading::GetCurrentProcess},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            DEVICE_NOTIFY_WINDOW_HANDLE, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND,
            PBT_POWERSETTINGCHANGE, WM_NCDESTROY, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE,
            WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
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
    SessionDisplayOff(bool),
}

type Handler = Rc<dyn Fn(Event)>;
struct Listener {
    handler: Handler,
    display_notification: HPOWERNOTIFY,
}
thread_local! {
    static HANDLERS: RefCell<BTreeMap<usize, Listener>> = const { RefCell::new(BTreeMap::new()) };
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
            entry.insert(Listener {
                handler: Rc::new(handler),
                display_notification: 0,
            });
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
        // SAFETY: The live window receives the documented per-session display setting.
        let display_notification = unsafe {
            RegisterPowerSettingNotification(
                hwnd,
                &GUID_SESSION_DISPLAY_STATUS,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        };
        if display_notification == 0 {
            let error = io::Error::last_os_error();
            unsubscribe(hwnd);
            return Err(error);
        }
        let installed = HANDLERS.with(|handlers| {
            let mut handlers = handlers.borrow_mut();
            if let Some(listener) = handlers.get_mut(&(hwnd as usize)) {
                listener.display_notification = display_notification;
                true
            } else {
                false
            }
        });
        if !installed {
            // SAFETY: We still own the successful registration.
            unsafe { UnregisterPowerSettingNotification(display_notification) };
            return Err(io::Error::other(
                "Window closed during display registration",
            ));
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

fn remove_handler(hwnd: HWND) -> Option<Listener> {
    HANDLERS.with(|handlers| handlers.borrow_mut().remove(&(hwnd as usize)))
}

fn unsubscribe(hwnd: HWND) {
    if let Some(listener) = remove_handler(hwnd) {
        // SAFETY: Only the registered window thread calls this for the owned HWND.
        unsafe {
            if listener.display_notification != 0 {
                UnregisterPowerSettingNotification(listener.display_notification);
            }
            WTSUnRegisterSessionNotification(hwnd);
            RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID);
        }
    }
}

#[repr(C)]
#[derive(Default)]
struct DisplaySetting {
    guid: windows_sys::core::GUID,
    data_length: u32,
    state: u32,
}

fn display_event(setting: DisplaySetting) -> Option<Event> {
    let expected = GUID_SESSION_DISPLAY_STATUS;
    if setting.data_length != 4
        || setting.guid.data1 != expected.data1
        || setting.guid.data2 != expected.data2
        || setting.guid.data3 != expected.data3
        || setting.guid.data4 != expected.data4
    {
        return None;
    }
    match setting.state {
        0 => Some(Event::SessionDisplayOff(true)),
        1 | 2 => Some(Event::SessionDisplayOff(false)),
        _ => None,
    }
}

fn read_display_event(address: LPARAM) -> Option<Event> {
    let mut setting = DisplaySetting::default();
    let mut read = 0;
    let size = std::mem::size_of::<DisplaySetting>();
    // SAFETY: ReadProcessMemory validates the supplied address and writes only into
    // this fixed-size local buffer. All field bit patterns are valid integers.
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as *const _,
            (&mut setting as *mut DisplaySetting).cast(),
            size,
            &mut read,
        )
    };
    if ok == 0 || read != size {
        return None;
    }
    display_event(setting)
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
    } else if let Some(event) =
        if message == WM_POWERBROADCAST && wparam == PBT_POWERSETTINGCHANGE as WPARAM {
            read_display_event(lparam)
        } else {
            event(message, wparam)
        }
    {
        let handler = HANDLERS.with(|handlers| {
            handlers
                .borrow()
                .get(&(hwnd as usize))
                .map(|listener| Rc::clone(&listener.handler))
        });
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
    fn accepts_only_a_complete_session_display_payload() {
        for (state, expected) in [
            (0, Some(true)),
            (1, Some(false)),
            (2, Some(false)),
            (3, None),
        ] {
            let setting = DisplaySetting {
                guid: GUID_SESSION_DISPLAY_STATUS,
                data_length: 4,
                state,
            };
            assert_eq!(
                read_display_event((&setting as *const DisplaySetting) as LPARAM),
                expected.map(Event::SessionDisplayOff)
            );
        }
        assert_eq!(
            display_event(DisplaySetting {
                data_length: 4,
                ..DisplaySetting::default()
            }),
            None
        );
        assert_eq!(
            display_event(DisplaySetting {
                guid: GUID_SESSION_DISPLAY_STATUS,
                data_length: 1,
                state: 0
            }),
            None
        );
        assert_eq!(read_display_event(0), None);
    }

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
