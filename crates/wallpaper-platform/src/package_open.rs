//! Open one package path component relative to an existing directory handle.
use std::{
    fs::File,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle},
    ptr,
};
use wallpaper_format::RelativePath;
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
            FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
        },
    },
    Win32::{
        Foundation::{OBJ_DONT_REPARSE, RtlNtStatusToDosError, UNICODE_STRING},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
            FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_SHARE_READ, FILE_TYPE_DISK,
            GetFileInformationByHandle, GetFileType, SYNCHRONIZE,
        },
        System::IO::IO_STATUS_BLOCK,
    },
};

#[derive(Clone, Copy)]
pub enum Kind {
    Directory,
    RegularFile,
}

/// The caller must establish the origin of the initial parent handle. Each call
/// opens exactly one component, with no absolute path or intermediate lookup.
pub fn open_child(parent: &File, component: &str, kind: Kind) -> io::Result<File> {
    if component.contains('/') || RelativePath::parse(component).is_err() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid package component",
        ));
    }
    check_handle(parent, Kind::Directory)?;
    let mut wide: Vec<u16> = component.encode_utf16().collect();
    let length = (wide.len() * size_of::<u16>()) as u16;
    let name = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: wide.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &name,
        Attributes: OBJ_DONT_REPARSE,
        SecurityDescriptor: ptr::null(),
        SecurityQualityOfService: ptr::null(),
    };
    let options = match kind {
        Kind::Directory => FILE_DIRECTORY_FILE,
        Kind::RegularFile => FILE_NON_DIRECTORY_FILE,
    };
    let mut handle = ptr::null_mut();
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: Both outputs are writable. The borrowed parent, attributes, and
    // UTF-16 buffer remain alive for this synchronous open. FILE_OPEN never creates an object.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &attributes,
            &mut status_block,
            ptr::null(),
            0,
            FILE_SHARE_READ,
            FILE_OPEN,
            options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            0,
        )
    };
    if status < 0 {
        // SAFETY: Any NTSTATUS value can be translated without pointer arguments.
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(status) } as i32,
        ));
    }
    if handle.is_null() {
        return Err(io::Error::other("Missing package file handle"));
    }
    // SAFETY: The successful native call returned a new owned file handle.
    let file = unsafe { File::from_raw_handle(handle) };
    check_handle(&file, kind)?;
    Ok(file)
}

fn check_handle(file: &File, kind: Kind) -> io::Result<()> {
    let handle = file.as_raw_handle();
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: The handle is borrowed from a live File and the output is valid.
    unsafe {
        if GetFileType(handle) != FILE_TYPE_DISK {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected disk handle",
            ));
        }
        if GetFileInformationByHandle(handle, &mut info) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let directory = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || directory != matches!(kind, Kind::Directory)
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Rejected package file type or link",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Read, os::windows::fs::OpenOptionsExt, process::Command, time::SystemTime};
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;

    #[test]
    fn relative_open_keeps_parent_identity_and_rejects_links() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "sahne-package-open-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root)?;
        let parent_path = root.join("parent");
        let original = root.join("original");
        fs::create_dir(&parent_path)?;
        fs::write(parent_path.join("entry.html"), b"original")?;
        let parent = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(&parent_path)?;
        assert!(open_child(&parent, "../entry.html", Kind::RegularFile).is_err());
        assert!(open_child(&parent, "entry.html", Kind::Directory).is_err());
        fs::rename(&parent_path, &original)?;
        fs::create_dir(&parent_path)?;
        fs::write(parent_path.join("entry.html"), b"replacement")?;
        let mut bytes = Vec::new();
        open_child(&parent, "entry.html", Kind::RegularFile)?.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"original");

        fs::hard_link(original.join("entry.html"), original.join("alias.html"))?;
        assert!(open_child(&parent, "alias.html", Kind::RegularFile).is_err());
        fs::create_dir(original.join("child"))?;
        open_child(&parent, "child", Kind::Directory)?;
        let junction = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J", "original\\junction", "parent"])
            .current_dir(&root)
            .output()?;
        assert!(
            junction.status.success(),
            "junction fixture creation failed"
        );
        assert!(open_child(&parent, "junction", Kind::Directory).is_err());

        drop(parent);
        fs::remove_dir(original.join("junction"))?;
        fs::remove_dir(original.join("child"))?;
        fs::remove_file(original.join("alias.html"))?;
        fs::remove_file(original.join("entry.html"))?;
        fs::remove_file(parent_path.join("entry.html"))?;
        fs::remove_dir(original)?;
        fs::remove_dir(parent_path)?;
        fs::remove_dir(root)
    }
}
