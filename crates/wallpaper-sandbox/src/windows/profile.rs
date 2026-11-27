use super::*;
use std::{
    ffi::OsString,
    fs,
    os::windows::{
        ffi::OsStringExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Storage::FileSystem::{FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT},
    System::Com::CoTaskMemFree,
};

const RECOVERY_INDEX: &str = "pending-profiles";
const MAX_RECOVERY_ENTRIES: usize = 128;

pub(super) struct Profile {
    name: Vec<u16>,
    pub(super) sid: PSID,
    storage: Option<PathBuf>,
    deleted: bool,
    // Keep the run's ownership record exclusively open through profile cleanup.
    lease: fs::File,
    pending: Option<PathBuf>,
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
        let ownership = format!("sahne-profile-v2\n{text}\n{token:016x}\n");
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
            lease,
            pending: None,
        };
        let (storage, root) = profile_storage(profile.sid, &text)?;
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
        profile.storage = Some(root);
        profile.index_run(run)?;
        Ok(profile)
    }

    pub(super) fn record_launch_intent(&mut self) -> io::Result<()> {
        writeln!(self.lease, "launching")?;
        self.lease.sync_all()
    }

    pub(super) fn record_worker(&mut self, pid: u32) -> io::Result<()> {
        if pid == 0 {
            return Err(io::Error::other("Invalid AppContainer worker PID"));
        }
        writeln!(self.lease, "worker={pid}")?;
        self.lease.sync_all()
    }

    fn index_run(&mut self, run: &Path) -> io::Result<()> {
        let Some(run_name) = run.file_name().and_then(|name| name.to_str()) else {
            return Ok(());
        };
        if !is_preview_run(run_name) {
            return Ok(());
        }
        let base = preview_run_base()?;
        if run.parent() != Some(base.as_path()) {
            return Ok(());
        }
        let index = base.join(RECOVERY_INDEX);
        match fs::create_dir(&index) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        check_directory(&index)?;
        let marker = index.join(run_name);
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)?;
        self.pending = Some(marker);
        file.sync_all()
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
        self.clear_pending();
        Ok(())
    }

    fn clear_pending(&mut self) {
        if let Some(path) = self.pending.take()
            && let Err(error) = fs::remove_file(path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("Could not clear AppContainer recovery marker: {error}");
        }
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
                } else {
                    self.clear_pending();
                }
            }
            FreeSid(self.sid);
        }
    }
}

fn profile_storage(sid: PSID, text: &str) -> io::Result<(PathBuf, PathBuf)> {
    let mut sid_text = ptr::null_mut();
    // SAFETY: This OS-created SID is live; the returned SID string uses LocalFree.
    check(unsafe { ConvertSidToStringSidW(sid, &mut sid_text) })?;
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
            .is_some_and(|name| name.eq_ignore_ascii_case(text))
    {
        return Err(io::Error::other(format!(
            "AppContainer storage path identity mismatch: expected={text}, actual={:?}",
            root.file_name()
        )));
    }
    for directory in [storage.as_path(), root] {
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::other(
                "AppContainer storage positive control is not a directory",
            ));
        }
    }
    let root = root.to_path_buf();
    Ok((storage, root))
}

struct DerivedSid(PSID);
impl Drop for DerivedSid {
    fn drop(&mut self) {
        // SAFETY: Owns the SID returned by DeriveAppContainerSidFromAppContainerName.
        unsafe { FreeSid(self.0) };
    }
}

fn read_record(file: &fs::File) -> io::Result<String> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other("Expected a regular profile record"));
    }
    let mut bytes = Vec::new();
    file.take(129).read_to_end(&mut bytes)?;
    if bytes.len() > 128 {
        return Err(io::Error::other("Profile record exceeds size limit"));
    }
    String::from_utf8(bytes).map_err(|_| io::Error::other("Invalid profile record encoding"))
}

fn process_exited(pid: u32) -> io::Result<bool> {
    // SAFETY: Requests a synchronization-only handle for a strictly parsed PID.
    let handle = match unsafe { own(OpenProcess(SYNCHRONIZE, 0, pid)) } {
        Err(error) if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) => {
            return Ok(true);
        }
        result => result?,
    };
    // SAFETY: Zero-time query on the owned process handle.
    match unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(io::Error::last_os_error()),
    }
}

fn parse_profile_record<'a>(
    record: &'a str,
    profile: &str,
    host: u32,
) -> io::Result<(&'a str, Option<u32>)> {
    let invalid = || io::Error::other("Invalid preview profile ownership record");
    let v2 = record.starts_with("sahne-profile-v2\n");
    let version = if v2 {
        "sahne-profile-v2"
    } else {
        "sahne-profile-v1"
    };
    let prefix = format!("{version}\n{profile}\n");
    let (token, state) = record
        .strip_prefix(&prefix)
        .and_then(|rest| rest.split_once('\n'))
        .ok_or_else(invalid)?;
    let token_id = u64::from_str_radix(token, 16).map_err(|_| invalid())?;
    if token != format!("{token_id:016x}") {
        return Err(invalid());
    }
    let ownership = &record[..prefix.len() + token.len() + 1];
    if v2 && state.is_empty() {
        return Ok((ownership, None));
    }
    let worker_state = if v2 {
        state.strip_prefix("launching\n").ok_or_else(invalid)?
    } else {
        state
    };
    let worker = worker_state
        .strip_prefix("worker=")
        .and_then(|line| line.strip_suffix('\n'))
        .ok_or_else(invalid)?;
    let pid = worker
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid != 0 && *pid != host)
        .ok_or_else(invalid)?;
    if worker != pid.to_string() {
        return Err(invalid());
    }
    Ok((ownership, Some(pid)))
}

fn is_preview_run(name: &str) -> bool {
    name.starts_with("preview-") || name.starts_with("intake-")
}

fn check_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other("Unexpected preview recovery directory"));
    }
    Ok(())
}

fn preview_run_base() -> io::Result<PathBuf> {
    let mut path = std::env::current_dir()?;
    for component in ["target", "sandbox-runs"] {
        path.push(component);
        check_directory(&path)?;
    }
    Ok(path)
}

/// Recover a bounded set of indexed profiles for this preview working directory.
pub fn recover_pending_preview_profiles() -> io::Result<usize> {
    let entries = match preview_run_base().and_then(|base| {
        let index = base.join(RECOVERY_INDEX);
        check_directory(&index)?;
        fs::read_dir(index)
    }) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        result => result?,
    };
    let mut recovered = 0;
    for entry in entries.take(MAX_RECOVERY_ENTRIES) {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        if let Some(name) = name.to_str()
            && recover_preview_profile(name).is_ok()
        {
            recovered += 1;
        }
    }
    Ok(recovered)
}

/// Recover a single preview or intake run from the current workspace.
pub fn recover_preview_profile(run_name: &str) -> io::Result<()> {
    let invalid = || io::Error::other("Invalid preview profile ownership record");
    let suffix = run_name
        .strip_prefix("preview-")
        .or_else(|| run_name.strip_prefix("intake-"))
        .ok_or_else(invalid)?;
    let (host, session) = suffix.split_once('-').ok_or_else(invalid)?;
    let host_pid = host
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid != 0)
        .ok_or_else(invalid)?;
    let session_id = u64::from_str_radix(session, 16).map_err(|_| invalid())?;
    if host != host_pid.to_string() || session != format!("{session_id:016x}") {
        return Err(invalid());
    }
    let base = preview_run_base()?;
    let run = base.join(run_name);
    check_directory(&run)?;
    let lease = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(run.join("profile-lease"))?;
    let record = read_record(&lease)?;
    let profile = format!("Sahne.Spike.{host_pid}.{session_id:016x}");
    let (ownership, worker_pid) = parse_profile_record(&record, &profile, host_pid)?;
    if !process_exited(host_pid)? || !worker_pid.map_or(Ok(true), process_exited)? {
        return Err(io::Error::other("Preview profile still has a live process"));
    }
    let name = wide(&profile)?;
    let mut sid = ptr::null_mut();
    // SAFETY: The validated profile name and SID output remain live for this call.
    hresult(unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) })?;
    let sid = DerivedSid(sid);
    let (storage, root) = profile_storage(sid.0, &profile)?;
    let control = fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(storage.join("sahne-cleanup-control.txt"))?;
    if read_record(&control)? != ownership {
        return Err(invalid());
    }
    drop(control);
    // SAFETY: Both run records match the exact profile and neither recorded process is alive.
    hresult(unsafe { DeleteAppContainerProfile(name.as_ptr()) })?;
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let index = base.join(RECOVERY_INDEX);
            if check_directory(&index).is_ok() {
                let _ = fs::remove_file(index.join(run_name));
            }
            Ok(())
        }
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::other(
            "AppContainer storage remains after recovery",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_profile_record;

    #[test]
    fn only_versioned_prelaunch_or_complete_worker_records_are_recoverable() {
        let profile = "Sahne.Spike.7.0000000000000001";
        let v1 = format!("sahne-profile-v1\n{profile}\n0000000000000002\n");
        let v2 = v1.replace("profile-v1", "profile-v2");
        assert!(parse_profile_record(&v1, profile, 7).is_err());
        assert_eq!(
            parse_profile_record(&v2, profile, 7).unwrap(),
            (v2.as_str(), None)
        );
        for suffix in [
            "launch",
            "launching\n",
            "launching\nworker=9",
            "worker=9\n",
            "launching\nworker=7\n",
        ] {
            assert!(parse_profile_record(&format!("{v2}{suffix}"), profile, 7).is_err());
        }
        for (ownership, state) in [(&v1, "worker=9\n"), (&v2, "launching\nworker=9\n")] {
            let record = format!("{ownership}{state}");
            assert_eq!(
                parse_profile_record(&record, profile, 7).unwrap(),
                (ownership.as_str(), Some(9))
            );
        }
    }
}
