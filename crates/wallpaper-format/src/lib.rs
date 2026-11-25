//! Bounded native manifest and immutable in-memory package. No filesystem/archive I/O.
use std::{collections::BTreeMap, fmt};
mod manifest;
mod path;
mod snapshot;
pub use manifest::Manifest;
pub use path::RelativePath;
pub use snapshot::SnapshotError;

pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
pub const MAX_ENTRIES: usize = 1024;
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
pub const FORMAT_VERSION: u16 = 1;
pub const API_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    BudgetExceeded,
    InvalidPath,
    CaseCollision,
    PathConflict,
    MissingManifest,
    ManifestSyntax,
    ManifestSchema,
    InvalidUtf8,
    UnsupportedVersion,
    InvalidId,
    InvalidName,
    InvalidFps,
    MissingSource,
    SourceType,
}
/// No untrusted JSON/path/source text is stored or echoed in diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: ErrorCode,
    pub entry_index: Option<usize>,
    pub line_column: Option<(usize, usize)>,
    pub field: Option<&'static str>,
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Package {:?}", self.code)?;
        if let Some(index) = self.entry_index {
            write!(f, " at entry {index}")?;
        }
        if let Some((line, column)) = self.line_column {
            write!(f, " at {line}:{column}")?;
        }
        if let Some(field) = self.field {
            write!(f, " ({field})")?;
        }
        Ok(())
    }
}
impl std::error::Error for Diagnostic {}
pub type Result<T> = std::result::Result<T, Diagnostic>;
fn diagnostic(code: ErrorCode) -> Diagnostic {
    Diagnostic {
        code,
        entry_index: None,
        line_column: None,
        field: None,
    }
}

#[derive(Debug)]
pub struct SourceFile {
    path: RelativePath,
    bytes: Vec<u8>,
}
impl SourceFile {
    pub fn path(&self) -> &RelativePath {
        &self.path
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn utf8(&self) -> Result<&str> {
        std::str::from_utf8(&self.bytes).map_err(|_| diagnostic(ErrorCode::InvalidUtf8))
    }
}

/// Captures owned bytes once. A future filesystem/helper collector must establish
/// regular-file/no-reparse provenance; this constructor does not certify a disk tree.
#[derive(Debug)]
pub struct Package {
    manifest: Manifest,
    files: BTreeMap<RelativePath, SourceFile>,
}
impl Package {
    pub fn from_entries(entries: Vec<(String, Vec<u8>)>) -> Result<Self> {
        check_sizes(entries.iter().map(|(_, bytes)| bytes.len()))?;
        let mut files = BTreeMap::new();
        let mut folded = BTreeMap::<String, usize>::new();
        for (index, (path, bytes)) in entries.into_iter().enumerate() {
            let path = RelativePath::parse(&path).map_err(|mut error| {
                error.entry_index = Some(index);
                error
            })?;
            let key = path.as_str().to_ascii_lowercase();
            if folded.contains_key(&key) {
                let mut error = diagnostic(ErrorCode::CaseCollision);
                error.entry_index = Some(index);
                return Err(error);
            }
            let mut parent = key.as_str();
            while let Some((prefix, _)) = parent.rsplit_once('/') {
                if folded.contains_key(prefix) {
                    let mut error = diagnostic(ErrorCode::PathConflict);
                    error.entry_index = Some(index);
                    return Err(error);
                }
                parent = prefix;
            }
            let prefix = format!("{key}/");
            if folded
                .range(prefix.clone()..)
                .next()
                .is_some_and(|(existing, _)| existing.starts_with(&prefix))
            {
                let mut error = diagnostic(ErrorCode::PathConflict);
                error.entry_index = Some(index);
                return Err(error);
            }
            folded.insert(key, index);
            files.insert(path.clone(), SourceFile { path, bytes });
        }
        let manifest_path = RelativePath::parse("manifest.json")?;
        let manifest_file = files
            .get(&manifest_path)
            .ok_or(diagnostic(ErrorCode::MissingManifest))?;
        let manifest = Manifest::parse(manifest_file.bytes())?;
        let package = Self { manifest, files };
        package.require_text(package.manifest.entry(), ".html", "entry")?;
        if let Some(script) = package.manifest.script() {
            package.require_text(script, ".js", "script")?;
        }
        Ok(package)
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn files(&self) -> impl ExactSizeIterator<Item = &SourceFile> {
        self.files.values()
    }
    pub fn file(&self, path: &RelativePath) -> Option<&SourceFile> {
        self.files.get(path)
    }
    pub fn require_text(
        &self,
        path: &RelativePath,
        extension: &str,
        field: &'static str,
    ) -> Result<&str> {
        let failure = |code| {
            let mut error = diagnostic(code);
            error.field = Some(field);
            error
        };
        if !path.as_str().ends_with(extension) {
            return Err(failure(ErrorCode::SourceType));
        }
        self.files
            .get(path)
            .ok_or(failure(ErrorCode::MissingSource))?
            .utf8()
            .map_err(|mut error| {
                error.field = Some(field);
                error
            })
    }
}

fn check_sizes(sizes: impl Iterator<Item = usize>) -> Result<()> {
    let mut total = 0usize;
    let mut count = 0usize;
    for size in sizes {
        count += 1;
        if count > MAX_ENTRIES || size > MAX_FILE_BYTES {
            return Err(diagnostic(ErrorCode::BudgetExceeded));
        }
        total = total
            .checked_add(size)
            .ok_or(diagnostic(ErrorCode::BudgetExceeded))?;
        if total > MAX_PACKAGE_BYTES {
            return Err(diagnostic(ErrorCode::BudgetExceeded));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
