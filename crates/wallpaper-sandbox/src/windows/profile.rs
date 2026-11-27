use super::*;
use std::{
    ffi::OsString,
    fs,
    os::windows::{ffi::OsStringExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
};
use windows_sys::Win32::System::Com::CoTaskMemFree;

pub(super) struct Profile {
    name: Vec<u16>,
    pub(super) sid: PSID,
    storage: Option<PathBuf>,
    deleted: bool,
    // Keep the run's ownership record exclusively open through profile cleanup.
    _lease: fs::File,
}

struct TaskAllocation(*mut u16);
impl Drop for TaskAllocation {
    fn drop(&mut self) {
        // SAFETY: GetAppContainerFolderPath allocates its string with the task allocator.
        unsafe { CoTaskMemFree(self.0.cast()) };
    }
}

impl Profile {
    pub(super) fn new(id: u64, run: &Path) -> io::Result<Self> {
        let text = format!("Sahne.Spike.{}.{id:016x}", std::process::id());
        let token = random_id()?;
        let ownership = format!("sahne-profile-v1\n{text}\n{token:016x}\n");
        let mut lease = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(run.join("profile-lease"))?;
        lease.write_all(ownership.as_bytes())?;
        lease.sync_all()?;
        let name = wide(&text)?;
        let mut sid = ptr::null_mut();
        // SAFETY: Terminated strings/output remain valid; no capabilities are requested.
        hresult(unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                ptr::null(),
                0,
                &mut sid,
            )
        })?;
        // Own the newly created profile before any fallible storage lookup.
        let mut profile = Self {
            name,
            sid,
            storage: None,
            deleted: false,
            _lease: lease,
        };
        let mut sid_text = ptr::null_mut();
        // SAFETY: This OS-created SID is live; the returned SID string uses LocalFree.
        check(unsafe { ConvertSidToStringSidW(profile.sid, &mut sid_text) })?;
        let _sid_text = LocalAllocation(sid_text.cast());
        let mut folder = ptr::null_mut();
        // SAFETY: Terminated SID string and initialized output; independent task allocation.
        hresult(unsafe { GetAppContainerFolderPath(sid_text, &mut folder) })?;
        let folder = TaskAllocation(folder);
        if folder.0.is_null() {
            return Err(io::Error::other("Null AppContainer storage path"));
        }
        let mut length = 0;
        // SAFETY: Successful OS API returns a terminated UTF-16 string, owned above.
        // The scan is capped at the Windows extended-path size; no borrowed string escapes.
        unsafe {
            while length < 32768 && *folder.0.add(length) != 0 {
                length += 1;
            }
        }
        if length == 0 || length == 32768 {
            return Err(io::Error::other("Invalid AppContainer storage path length"));
        }
        // SAFETY: The scanned initialized UTF-16 prefix is valid until folder is dropped.
        let storage = PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(folder.0, length)
        }));
        // Windows returns the profile's AC local-data child on this configuration.
        // Bind the observation to the exact owned profile root, never a shared ancestor.
        let root = if storage.file_name().is_some_and(|name| name == "AC") {
            storage
                .parent()
                .ok_or_else(|| io::Error::other("Missing profile root"))?
        } else {
            storage.as_path()
        };
        if !root.is_absolute()
            || !root
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case(&text))
        {
            return Err(io::Error::other(format!(
                "AppContainer storage path identity mismatch: expected={text}, actual={:?}",
                root.file_name()
            )));
        }
        for directory in [storage.as_path(), root] {
            let metadata = fs::symlink_metadata(directory)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(io::Error::other(
                    "AppContainer storage positive control is not a directory",
                ));
            }
        }
        // A fresh nonempty storage control proves cleanup removes data, not only an
        // already-empty profile directory. Close it before launching/deleting the profile.
        let canary = storage.join("sahne-cleanup-control.txt");
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&canary)?;
            file.write_all(ownership.as_bytes())?;
            file.sync_all()?;
        }
        if fs::read(&canary)? != ownership.as_bytes() {
            return Err(io::Error::other("Profile storage content control failed"));
        }
        profile.storage = Some(root.to_path_buf());
        Ok(profile)
    }

    pub(super) fn cleanup(&mut self) -> io::Result<()> {
        if self.deleted {
            return Ok(());
        }
        let storage = self
            .storage
            .as_ref()
            .ok_or_else(|| io::Error::other("Missing AppContainer storage positive control"))?;
        // SAFETY: Delete only the unique profile successfully created/owned by this instance.
        // The host has already stopped/waited the worker and joined its reader.
        hresult(unsafe { DeleteAppContainerProfile(self.name.as_ptr()) })?;
        match fs::symlink_metadata(storage) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
            Ok(_) => {
                return Err(io::Error::other(
                    "AppContainer storage remains after deletion",
                ));
            }
        }
        self.deleted = true;
        Ok(())
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        // SAFETY: The unique profile was created here; SID was allocated by CreateAppContainerProfile.
        // Failed explicit cleanup/partial construction retains a best-effort OS deletion fallback.
        unsafe {
            if !self.deleted {
                let result = DeleteAppContainerProfile(self.name.as_ptr());
                if result < 0 {
                    eprintln!("AppContainer profile cleanup failed: {result:#x}");
                }
            }
            FreeSid(self.sid);
        }
    }
}
