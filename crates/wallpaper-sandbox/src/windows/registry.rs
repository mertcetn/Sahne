use super::*;
use windows_sys::Win32::{
    Storage::FileSystem::{DELETE, WRITE_OWNER},
    System::Registry::*,
};

const DATA: &[u8] = b"Sahne synthetic registry sentinel";
const RIGHTS: [u32; 8] = [
    KEY_QUERY_VALUE,
    KEY_SET_VALUE,
    KEY_CREATE_SUB_KEY,
    KEY_ENUMERATE_SUB_KEYS,
    DELETE,
    READ_CONTROL,
    WRITE_DAC,
    WRITE_OWNER,
];
const VIEW: u32 = KEY_WOW64_64KEY;

// Registry handles must use RegCloseKey, never Foundation::CloseHandle.
struct Key(HKEY);
impl Key {
    fn close(&mut self) -> io::Result<()> {
        if !self.0.is_null() {
            // SAFETY: Owns this non-predefined key returned by a successful registry API.
            winerror(unsafe { RegCloseKey(self.0) })?;
            self.0 = ptr::null_mut();
        }
        Ok(())
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("Registry handle cleanup failed: {error}");
        }
    }
}
fn name(host: u32, session: u64) -> String {
    format!("Sahne.Probe.{host}.{session:016x}")
}
fn open(parent: HKEY, name: &[u16], access: u32) -> io::Result<Key> {
    let mut key = ptr::null_mut();
    // SAFETY: Live/predefined parent, terminated nonempty name, initialized output.
    winerror(unsafe { RegOpenKeyExW(parent, name.as_ptr(), 0, access | VIEW, &mut key) })?;
    if key.is_null() {
        return Err(io::Error::other("Null registry handle"));
    }
    Ok(Key(key))
}
fn create(parent: HKEY, name: &[u16], security: *const SECURITY_ATTRIBUTES) -> io::Result<Key> {
    let (mut key, mut disposition) = (ptr::null_mut(), 0);
    // SAFETY: Only fixed children of an owned fixture or the fresh unique Software name;
    // supplied descriptor/ACL remain alive. No link, backup or restore options are used.
    winerror(unsafe {
        RegCreateKeyExW(
            parent,
            name.as_ptr(),
            0,
            ptr::null(),
            REG_OPTION_VOLATILE,
            KEY_ALL_ACCESS | VIEW,
            security,
            &mut key,
            &mut disposition,
        )
    })?;
    if key.is_null() {
        return Err(io::Error::other("Null created registry handle"));
    }
    let key = Key(key);
    if disposition != REG_CREATED_NEW_KEY {
        return Err(io::Error::other(
            "Refusing an existing registry fixture key",
        ));
    }
    Ok(key)
}
fn delete(parent: HKEY, name: &[u16]) -> io::Result<()> {
    // SAFETY: Deletes only an explicitly owned unique key or its fixed test child.
    // This non-recursive API cannot delete a tree containing unknown subkeys.
    winerror(unsafe { RegDeleteKeyExW(parent, name.as_ptr(), VIEW, 0) })
}
fn set_sentinel(key: &Key) -> io::Result<()> {
    let value = wide("Sentinel")?;
    // SAFETY: Owned writable fixture, live terminated name and fixed bounded binary bytes.
    winerror(unsafe {
        RegSetValueExW(
            key.0,
            value.as_ptr(),
            0,
            REG_BINARY,
            DATA.as_ptr(),
            DATA.len() as u32,
        )
    })
}

pub(super) struct RegistryFixture {
    parent: Key,
    key: Option<Key>,
    name: Vec<u16>,
    deleted: bool,
}
impl RegistryFixture {
    pub(super) fn new(session: u64) -> io::Result<Self> {
        let parent = open(HKEY_CURRENT_USER, &wide("Software")?, KEY_READ | KEY_WRITE)?;
        let name = wide(name(std::process::id(), session))?;
        let acl = host_user_acl_with_rights(KEY_ALL_ACCESS)?;
        let mut descriptor = SECURITY_DESCRIPTOR::default();
        let descriptor_ptr = &mut descriptor as *mut _ as PSECURITY_DESCRIPTOR;
        // SAFETY: Initialize first, then apply the live non-null host-user-only ACL.
        unsafe {
            check(InitializeSecurityDescriptor(descriptor_ptr, 1))?;
            check(SetSecurityDescriptorDacl(
                descriptor_ptr,
                1,
                acl.0.cast(),
                0,
            ))?;
            check(SetSecurityDescriptorControl(
                descriptor_ptr,
                SE_DACL_PROTECTED,
                SE_DACL_PROTECTED,
            ))?;
        }
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor_ptr,
            bInheritHandle: 0,
        };
        let key = create(parent.0, &name, &security)?;
        // Install cleanup ownership only after disposition proves this instance created it.
        let fixture = Self {
            parent,
            key: Some(key),
            name,
            deleted: false,
        };
        let key = fixture.key.as_ref().unwrap();
        set_sentinel(key)?;
        for access in RIGHTS {
            open(fixture.parent.0, &fixture.name, access)?.close()?;
        }
        let control_name = wide("host-control")?;
        let mut control = create(key.0, &control_name, &security)?;
        set_sentinel(&control)?;
        control.close()?;
        delete(key.0, &control_name)?;
        fixture.verify_contents()?;
        println!(
            "Registry host controls: eight access opens and child create/delete succeeded; protected user-only volatile fixture, 64-bit view; no key handle inherits."
        );
        Ok(fixture)
    }
    fn verify_contents(&self) -> io::Result<()> {
        let key = self
            .key
            .as_ref()
            .ok_or_else(|| io::Error::other("Registry fixture already closed"))?;
        let value = wide("Sentinel")?;
        let (mut kind, mut length) = (0, DATA.len() as u32);
        let mut bytes = [0u8; DATA.len()];
        let (mut subkeys, mut values) = (0, 0);
        // SAFETY: Live owned key; all output buffers/counts match their exact sizes.
        // Oversized/changed values fail without allocating attacker-controlled sizes.
        unsafe {
            winerror(RegQueryValueExW(
                key.0,
                value.as_ptr(),
                ptr::null(),
                &mut kind,
                bytes.as_mut_ptr(),
                &mut length,
            ))?;
            winerror(RegQueryInfoKeyW(
                key.0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
                &mut subkeys,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut values,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            ))?;
        }
        if kind != REG_BINARY
            || length as usize != DATA.len()
            || bytes != DATA
            || subkeys != 0
            || values != 1
        {
            return Err(io::Error::other(
                "Registry fixture contents/namespace changed",
            ));
        }
        Ok(())
    }
    pub(super) fn cleanup(&mut self) -> io::Result<()> {
        if self.deleted {
            return Ok(());
        }
        self.verify_contents()?;
        self.key.as_mut().unwrap().close()?;
        self.key.take();
        delete(self.parent.0, &self.name)?;
        match open(self.parent.0, &self.name, KEY_READ) {
            Err(error) if error.raw_os_error() == Some(2) => {}
            Err(error) => return Err(error),
            Ok(_) => return Err(io::Error::other("Registry fixture remains after deletion")),
        }
        self.parent.close()?;
        self.deleted = true;
        Ok(())
    }
}
impl Drop for RegistryFixture {
    fn drop(&mut self) {
        if self.deleted {
            return;
        }
        // Best-effort fallback for failed construction/probes. Only the two fixed child
        // names and this instance's unique owned root may be deleted; no recursive sweep.
        if let Some(key) = self.key.as_ref() {
            for child in ["host-control", "worker-created"] {
                if let Ok(child) = wide(child) {
                    let _ = delete(key.0, &child);
                }
            }
        }
        self.key.take();
        if let Err(error) = delete(self.parent.0, &self.name)
            && error.raw_os_error() != Some(2)
        {
            eprintln!("Registry fixture cleanup failed: {error}");
        }
    }
}

pub fn probe_registry(host: u32, session: u64) -> io::Result<()> {
    if !is_appcontainer()? {
        return Err(io::Error::other("Registry probe requires AppContainer"));
    }
    let path = format!("Software\\{}", name(host, session));
    let key_name = wide(&path)?;
    for access in RIGHTS {
        match open(HKEY_CURRENT_USER, &key_name, access) {
            Err(error) if error.raw_os_error() == Some(5) => {}
            Err(error) => {
                return Err(io::Error::other(format!(
                    "Registry access {access:#x}: {error}"
                )));
            }
            Ok(_) => {
                return Err(io::Error::other(format!(
                    "Registry access unexpectedly allowed: {access:#x}"
                )));
            }
        }
    }
    let child = wide(format!("{path}\\worker-created"))?;
    match create(HKEY_CURRENT_USER, &child, ptr::null()) {
        Err(error) if error.raw_os_error() == Some(5) => {}
        other => {
            return Err(io::Error::other(format!(
                "Registry child creation was not denied: {:?}",
                other.err()
            )));
        }
    }
    match delete(HKEY_CURRENT_USER, &key_name) {
        Err(error) if error.raw_os_error() == Some(5) => Ok(()),
        other => Err(io::Error::other(format!(
            "Registry deletion was not denied: {other:?}"
        ))),
    }
}
