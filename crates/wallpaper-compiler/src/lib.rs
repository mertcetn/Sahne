//! Strict, bounded native source subset. No browser recovery, I/O or source execution.
use std::{collections::BTreeSet, fmt};
use wallpaper_format::RelativePath;
mod compile;
pub mod css;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
pub mod html;
pub use compile::{Compiled, CompiledGroups, Viewport, compile, compile_groups};

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_DEPTH: usize = 32;
pub const MAX_STYLESHEETS: usize = 16;
pub const MAX_RULES: usize = 1024;
pub const MAX_DECLARATIONS: usize = 65536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    BudgetExceeded,
    Syntax,
    UnsupportedElement,
    UnsupportedAttribute,
    DuplicateAttribute,
    DuplicateId,
    InvalidIdentifier,
    InvalidReference,
    MixedContent,
    MismatchedTag,
    UnsupportedSelector,
    UnsupportedProperty,
    UnsupportedValue,
    InvalidNumber,
    InvalidColor,
    MissingResource,
    MissingDimensions,
    MissingTransformOrigin,
    Image(wallpaper_media::Error),
    SceneValidation(wallpaper_scene::ErrorCode),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: ErrorCode,
    pub path: RelativePath,
    pub span: Span,
    /// One-based line and UTF-8 byte column in the original source.
    pub line: usize,
    pub column: usize,
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {:?}",
            self.path.as_str(),
            self.line,
            self.column,
            self.code
        )
    }
}
impl std::error::Error for Diagnostic {}
pub type Result<T> = std::result::Result<T, Diagnostic>;

#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub path: &'a RelativePath,
    pub text: &'a str,
}
impl Source<'_> {
    fn error(self, code: ErrorCode, span: Span) -> Diagnostic {
        let start = span.start.min(self.text.len());
        // All parser spans lie at UTF-8 boundaries; byte scanning also keeps
        // diagnostics safe if a later caller supplies an arbitrary byte offset.
        let prefix = &self.text.as_bytes()[..start];
        let line = prefix.iter().filter(|&&byte| byte == b'\n').count() + 1;
        let column = prefix
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(start + 1, |last| start - last);
        Diagnostic {
            code,
            path: self.path.clone(),
            span,
            line,
            column,
        }
    }
}
fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 64
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}
fn ascii_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | 0x0c)
}

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod tests;
