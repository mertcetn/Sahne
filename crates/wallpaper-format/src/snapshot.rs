use crate::{
    Diagnostic, ErrorCode, MAX_ENTRIES, MAX_FILE_BYTES, MAX_MANIFEST_BYTES, MAX_PACKAGE_BYTES,
    Package, RelativePath, diagnostic,
};
use std::{
    fmt,
    io::{self, Read},
};

#[derive(Debug)]
pub enum SnapshotError {
    Package(Diagnostic),
    Read {
        entry_index: usize,
        kind: io::ErrorKind,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(error) => error.fmt(f),
            Self::Read { entry_index, kind } => {
                write!(f, "Package read {kind:?} at entry {entry_index}")
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

fn budget(entry_index: usize) -> SnapshotError {
    let mut error = diagnostic(ErrorCode::BudgetExceeded);
    error.entry_index = Some(entry_index);
    SnapshotError::Package(error)
}

impl Package {
    /// Retain bytes from caller-owned readers. The caller establishes handle provenance,
    /// directory containment and operational deadlines before supplying readers.
    pub fn from_readers<R: Read>(
        entries: impl IntoIterator<Item = io::Result<(String, R)>>,
    ) -> Result<Self, SnapshotError> {
        let mut retained = Vec::new();
        let mut remaining = MAX_PACKAGE_BYTES;
        let mut buffer = [0; 8192];
        for (index, entry) in entries.into_iter().enumerate() {
            if index >= MAX_ENTRIES {
                return Err(budget(index));
            }
            let read_error = |error: io::Error| SnapshotError::Read {
                entry_index: index,
                kind: error.kind(),
            };
            let (path, mut reader) = entry.map_err(read_error)?;
            let relative = RelativePath::parse(&path).map_err(|mut error| {
                error.entry_index = Some(index);
                SnapshotError::Package(error)
            })?;
            let file_limit = if relative.as_str() == "manifest.json" {
                MAX_MANIFEST_BYTES
            } else {
                MAX_FILE_BYTES
            };
            let limit = remaining.min(file_limit);
            let mut bytes = Vec::new();
            loop {
                // The extra byte distinguishes an exact fit from an oversized stream.
                let count = buffer.len().min(limit - bytes.len() + 1);
                let read = match reader.read(&mut buffer[..count]) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    result => result.map_err(read_error)?,
                };
                if read == 0 {
                    break;
                }
                if read > limit - bytes.len() {
                    return Err(budget(index));
                }
                bytes.extend_from_slice(&buffer[..read]);
            }
            remaining -= bytes.len();
            retained.push((path, bytes));
        }
        Self::from_entries(retained).map_err(SnapshotError::Package)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn snapshot_owns_exact_bytes_and_still_validates_manifest_references() {
        let manifest = br#"{"formatVersion":1,"apiVersion":1,"id":"snapshot","name":"Snapshot","entry":"index.html","permissions":[],"preferredFps":30}"#;
        let html = b"<div></div>";
        let source = [
            Ok(("manifest.json".into(), Cursor::new(manifest.as_slice()))),
            Ok(("index.html".into(), Cursor::new(html.as_slice()))),
        ];
        let package = Package::from_readers(source).unwrap();
        assert_eq!(
            package.file(package.manifest().entry()).unwrap().bytes(),
            html
        );
        assert!(matches!(
            Package::from_readers([Ok(("manifest.json".into(), Cursor::new(manifest)))]),
            Err(SnapshotError::Package(Diagnostic {
                code: ErrorCode::MissingSource,
                ..
            }))
        ));
    }

    #[test]
    fn oversized_manifest_reads_at_most_one_extra_byte() {
        let mut consumed = 0;
        let reader = io::repeat(0);
        struct Count<'a, R>(&'a mut usize, R);
        impl<R: Read> Read for Count<'_, R> {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let n = self.1.read(buffer)?;
                *self.0 += n;
                Ok(n)
            }
        }
        assert!(matches!(
            Package::from_readers([Ok(("manifest.json".into(), Count(&mut consumed, reader)))]),
            Err(SnapshotError::Package(Diagnostic {
                code: ErrorCode::BudgetExceeded,
                ..
            }))
        ));
        assert_eq!(consumed, MAX_MANIFEST_BYTES + 1);
    }

    #[test]
    fn paths_and_read_failures_return_bounded_diagnostics() {
        assert!(matches!(
            Package::from_readers([Ok(("../index.html".into(), io::empty()))]),
            Err(SnapshotError::Package(Diagnostic {
                code: ErrorCode::InvalidPath,
                ..
            }))
        ));
        assert!(matches!(
            Package::from_readers::<io::Empty>([Err(io::Error::other("private path"))]),
            Err(SnapshotError::Read {
                entry_index: 0,
                kind: io::ErrorKind::Other
            })
        ));
    }
}
