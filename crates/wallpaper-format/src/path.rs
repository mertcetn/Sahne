use super::*;

/// Portable ASCII relative path. Slash-separated, exact-case lookup, no normalization.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelativePath(String);
impl RelativePath {
    pub fn parse(value: &str) -> Result<Self> {
        let invalid = || diagnostic(ErrorCode::InvalidPath);
        if value.is_empty() || value.len() > 240 {
            return Err(invalid());
        }
        let mut count = 0;
        for segment in value.split('/') {
            count += 1;
            if count > 32
                || segment.is_empty()
                || segment.len() > 64
                || segment == "."
                || segment == ".."
                || segment.ends_with('.')
                || !segment
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            {
                return Err(invalid());
            }
            let stem = segment.split('.').next().unwrap_or("").to_ascii_uppercase();
            if ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit())
            {
                return Err(invalid());
            }
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
