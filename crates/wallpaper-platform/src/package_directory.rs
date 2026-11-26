//! Host-side Windows directory collection. All descendant opens are handle-relative.
use crate::package_open::{Kind, open_child};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io,
    mem::offset_of,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
};
use wallpaper_format::{MAX_ENTRIES, Package, PackageSnapshot, RelativePath, SnapshotError};
use windows_sys::Win32::{
    Foundation::ERROR_NO_MORE_FILES,
    Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_FULL_DIR_INFO, FILE_SHARE_READ, FileFullDirectoryInfo,
        FileFullDirectoryRestartInfo, GetDriveTypeW, GetFileInformationByHandleEx,
        GetFinalPathNameByHandleW,
    },
};

/// Collect from an absolute fixed-drive path with portable ASCII components.
pub fn collect(path: &str) -> Result<Package, Box<dyn std::error::Error>> {
    Ok(collect_snapshot(path)?
        .parse()
        .map_err(SnapshotError::Package)?)
}

/// Capture bounded file bytes without parsing the package sources.
pub fn collect_snapshot(path: &str) -> Result<PackageSnapshot, Box<dyn std::error::Error>> {
    let mut dir = open_folder(path)?;
    let mut entries = Vec::new();
    let mut count = 0;
    walk(&mut dir, "", &mut count, &mut entries)?;
    Ok(PackageSnapshot::from_readers(entries.into_iter().map(Ok))?)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Open a fixed-drive folder after validating each parent component.
pub fn open_folder(path: &str) -> io::Result<File> {
    let bytes = path.as_bytes();
    if bytes.len() > 243
        || bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || &bytes[1..3] != b":\\"
    {
        return Err(invalid(
            "Expected an absolute package folder on a fixed drive",
        ));
    }
    let suffix = path[3..].replace('\\', "/");
    RelativePath::parse(&suffix).map_err(|_| invalid("Invalid package root components"))?;
    let drive = format!("{}:\\", char::from(bytes[0].to_ascii_uppercase()));
    let wide: Vec<u16> = drive.encode_utf16().chain(Some(0)).collect();
    // SAFETY: The drive name is a live, NUL-terminated UTF-16 string.
    const DRIVE_FIXED: u32 = 3;
    if unsafe { GetDriveTypeW(wide.as_ptr()) } != DRIVE_FIXED {
        return Err(invalid("Expected a fixed drive"));
    }
    let device_root = format!(r"\\?\{drive}");
    let mut directory = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&device_root)?;
    let mut actual = [0u16; 16];
    // SAFETY: The handle and writable UTF-16 buffer remain valid through the call.
    let count = unsafe {
        GetFinalPathNameByHandleW(
            directory.as_raw_handle(),
            actual.as_mut_ptr(),
            actual.len() as u32,
            0,
        )
    } as usize;
    if count == 0
        || count >= actual.len()
        || !String::from_utf16_lossy(&actual[..count]).eq_ignore_ascii_case(&device_root)
    {
        return Err(invalid("Drive root identity could not be verified"));
    }
    for component in suffix.split('/') {
        directory = open_child(&directory, component, Kind::Directory)?;
    }
    Ok(directory)
}

fn walk(
    dir: &mut File,
    prefix: &str,
    count: &mut usize,
    entries: &mut Vec<(String, File)>,
) -> io::Result<()> {
    let listing = list(dir, count)?;
    for (name, is_directory) in listing {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        RelativePath::parse(&path)
            .map_err(|_| invalid("Invalid or overlong package entry path"))?;
        if is_directory {
            let mut child = open_child(dir, &name, Kind::Directory)?;
            walk(&mut child, &path, count, entries)?;
        } else {
            entries.push((path, open_child(dir, &name, Kind::RegularFile)?));
        }
    }
    Ok(())
}

#[repr(align(8))]
struct DirectoryBuffer([u8; 8192]);

fn list(dir: &mut File, count: &mut usize) -> io::Result<Vec<(String, bool)>> {
    let mut buffer = DirectoryBuffer([0; 8192]);
    let mut class = FileFullDirectoryRestartInfo;
    let mut names = BTreeSet::new();
    let mut entries = Vec::new();
    loop {
        buffer.0.fill(0);
        // SAFETY: This directory handle is live and the aligned buffer is writable.
        let success = unsafe {
            GetFileInformationByHandleEx(
                dir.as_raw_handle(),
                class,
                buffer.0.as_mut_ptr().cast(),
                buffer.0.len() as u32,
            )
        };
        if success == 0 {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                Ok(entries)
            } else {
                Err(error)
            };
        }
        let restart = class == FileFullDirectoryRestartInfo;
        class = FileFullDirectoryInfo;
        let mut record = buffer.0.as_slice();
        loop {
            const NAME: usize = offset_of!(FILE_FULL_DIR_INFO, FileName);
            if record.len() < NAME {
                return Err(invalid("Incomplete directory record"));
            }
            let word = |offset| u32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
            let next = word(0) as usize;
            let length = word(offset_of!(FILE_FULL_DIR_INFO, FileNameLength)) as usize;
            let attributes = word(offset_of!(FILE_FULL_DIR_INFO, FileAttributes));
            let end = NAME
                .checked_add(length)
                .ok_or_else(|| invalid("Invalid directory record length"))?;
            if length == 0
                || !length.is_multiple_of(2)
                || end > record.len()
                || (next != 0 && (next < end || next >= record.len()))
            {
                return Err(invalid("Invalid directory record bounds"));
            }
            let name: String = record[NAME..end]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| {
                    if pair[1] == 0 && pair[0].is_ascii() {
                        Ok(char::from(pair[0]))
                    } else {
                        Err(invalid("Nonportable package entry name"))
                    }
                })
                .collect::<io::Result<_>>()?;
            if name == "." || name == ".." {
                if !restart {
                    return Err(invalid("Unexpected dot entry after directory restart"));
                }
            } else {
                *count += 1;
                if *count > MAX_ENTRIES || attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    return Err(invalid("Package directory entry limit or reparse point"));
                }
                if name.contains('/')
                    || RelativePath::parse(&name).is_err()
                    || !names.insert(name.to_ascii_lowercase())
                {
                    return Err(invalid("Invalid or colliding package entry name"));
                }
                entries.push((name, attributes & FILE_ATTRIBUTE_DIRECTORY != 0));
            }
            if next == 0 {
                break;
            }
            record = &record[next..];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn collects_nested_package_across_directory_batches_and_rejects_junctions()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "sahne-collect-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::create_dir(&root)?;
        let nested = root.join("web");
        fs::create_dir(&nested)?;
        fs::write(root.join("manifest.json"), br#"{"formatVersion":1,"apiVersion":1,"id":"snapshot","name":"Snapshot","entry":"web/index.html","permissions":[],"preferredFps":30}"#)?;
        fs::write(nested.join("index.html"), b"<div></div>")?;
        for index in 0..100 {
            fs::write(nested.join(format!("data-{index:03}.txt")), b"sample")?;
        }
        let path = root
            .to_str()
            .ok_or_else(|| invalid("Non-UTF-8 test path"))?;
        let package = collect(path)?;
        assert_eq!(package.files().len(), 102);
        assert_eq!(
            package.file(package.manifest().entry()).unwrap().bytes(),
            b"<div></div>"
        );
        assert!(collect(&format!("{path}\\..\\web")).is_err());
        let junction = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J", "alias", "web"])
            .current_dir(&root)
            .output()?;
        assert!(
            junction.status.success(),
            "junction fixture creation failed"
        );
        assert!(collect_snapshot(path).is_err());
        assert!(collect(&format!("{path}\\alias")).is_err());
        fs::remove_dir(root.join("alias"))?;
        for index in 0..100 {
            fs::remove_file(nested.join(format!("data-{index:03}.txt")))?;
        }
        fs::remove_file(nested.join("index.html"))?;
        fs::remove_dir(nested)?;
        fs::remove_file(root.join("manifest.json"))?;
        fs::remove_dir(root)?;
        Ok(())
    }
}
