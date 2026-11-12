use super::*;
use std::{fs, path::PathBuf};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_APPEND_DATA, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, WRITE_OWNER,
};

const PUBLIC: &[u8] = b"Sahne staged read-only fixture";
const PRIVATE: &[u8] = b"Sahne private synthetic fixture";

// Fixed synthetic targets only. Request rights without performing destructive operations.
fn denied_opens(root: &Path) -> Vec<(PathBuf, u32)> {
    let private = root.join("private/secret.txt");
    let public = root.join("bin/readable.txt");
    let mut requests = vec![(private.clone(), FILE_READ_DATA)];
    for path in [private, public] {
        for access in [
            FILE_WRITE_DATA,
            FILE_APPEND_DATA,
            DELETE,
            WRITE_DAC,
            WRITE_OWNER,
        ] {
            requests.push((path.clone(), access));
        }
    }
    requests
}

fn open(path: &Path, access: u32) -> io::Result<OwnedHandle> {
    let path = wide(path)?;
    // SAFETY: Fixed fixture path, no creation/truncation/delete-on-close; returned handle
    // is owned. Full sharing prevents an unrelated sharing denial satisfying the test.
    unsafe {
        own(CreateFileW(
            path.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        ))
    }
}

fn private_acl(path: &Path) -> io::Result<()> {
    let path = wide(path)?;
    let acl = host_user_acl()?;
    // SAFETY: Only this run's newly created synthetic directory/file is passed. The
    // non-null host-user ACL lives through the synchronous call; inheritance is blocked.
    winerror(unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            acl.0.cast(),
            ptr::null(),
        )
    })
}

pub(super) fn prepare(root: &Path) -> io::Result<()> {
    let directory = root.join("private");
    fs::create_dir(&directory)?;
    private_acl(&directory)?;
    let private = directory.join("secret.txt");
    fs::write(&private, PRIVATE)?;
    private_acl(&private)?;
    fs::write(root.join("bin/readable.txt"), PUBLIC)?;
    // Every requested file right must succeed for the host on that same live file.
    for (path, access) in denied_opens(root) {
        drop(open(&path, access).map_err(|e| {
            io::Error::other(format!("Filesystem host positive control {access:#x}: {e}"))
        })?);
    }
    if fs::read(&private)? != PRIVATE
        || fs::read_dir(&directory)?
            .collect::<io::Result<Vec<_>>>()?
            .len()
            != 1
    {
        return Err(io::Error::other(
            "Private filesystem fixture setup mismatch",
        ));
    }
    for directory in [root.join("private"), root.join("bin")] {
        let file = directory.join("host-control.txt");
        let folder = directory.join("host-control-dir");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)?;
        fs::remove_file(file)?;
        fs::create_dir(&folder)?;
        fs::remove_dir(folder)?;
    }
    println!(
        "Filesystem host controls: all eleven requested rights, private listing and file/directory creation succeeded on synthetic targets."
    );
    Ok(())
}

fn require_denial<T>(result: io::Result<T>, operation: &str) -> io::Result<()> {
    match result {
        Err(error) if error.raw_os_error() == Some(5) => Ok(()),
        Err(error) => Err(io::Error::other(format!(
            "{operation}: expected denial, got {error}"
        ))),
        Ok(_) => Err(io::Error::other(format!(
            "{operation}: unexpectedly allowed"
        ))),
    }
}

fn require_mz(path: &Path) -> io::Result<()> {
    let mut header = [0; 2];
    File::open(path)?.read_exact(&mut header)?;
    if &header != b"MZ" {
        return Err(io::Error::other("Permitted executable header mismatch"));
    }
    Ok(())
}

/// Trusted fixed-path filesystem matrix, not a general file API for scripts.
pub fn probe_filesystem(root: &Path) -> io::Result<()> {
    if !is_appcontainer()? {
        return Err(io::Error::other("Filesystem probe requires AppContainer"));
    }
    if fs::read(root.join("bin/readable.txt"))? != PUBLIC {
        return Err(io::Error::other("Staged read positive control failed"));
    }
    require_mz(&root.join("bin/probe.exe"))?;
    let system = std::env::var_os("SystemRoot")
        .ok_or_else(|| io::Error::other("Missing curated SystemRoot"))?;
    require_mz(&PathBuf::from(system).join("System32/kernel32.dll"))?;
    let listing = fs::read_dir(root.join("bin"))?.collect::<io::Result<Vec<_>>>()?;
    if listing.len() != 2 {
        return Err(io::Error::other("Staged directory listing mismatch"));
    }
    for (path, access) in denied_opens(root) {
        require_denial(open(&path, access), &format!("File access {access:#x}"))?;
    }
    require_denial(fs::read_dir(root.join("private")), "Private listing")?;
    for directory in [root.join("private"), root.join("bin")] {
        require_denial(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join("worker-created.txt")),
            "File creation",
        )?;
        require_denial(
            fs::create_dir(directory.join("worker-created-dir")),
            "Directory creation",
        )?;
    }
    Ok(())
}

/// Independent host content/namespace check after the worker has stopped.
pub fn verify_filesystem_fixture(root: &Path) -> io::Result<()> {
    require_mz(&root.join("bin/probe.exe"))?;
    if fs::read(root.join("private/secret.txt"))? != PRIVATE
        || fs::read(root.join("bin/readable.txt"))? != PUBLIC
    {
        return Err(io::Error::other("Filesystem fixture contents changed"));
    }
    for (directory, expected) in [(root.join("private"), 1), (root.join("bin"), 2)] {
        let entries = fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?;
        if entries.len() != expected {
            return Err(io::Error::other("Filesystem fixture namespace changed"));
        }
    }
    Ok(())
}
