use super::*;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawManifest {
    format_version: u16,
    api_version: u16,
    id: String,
    name: String,
    entry: String,
    #[serde(rename = "permissions", deserialize_with = "no_permissions")]
    _permissions: (),
    preferred_fps: u16,
    #[serde(default, deserialize_with = "script_field")]
    script: Option<String>,
}
fn no_permissions<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<(), D::Error> {
    struct Empty;
    impl<'de> serde::de::Visitor<'de> for Empty {
        type Value = ();
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an empty permissions array")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> std::result::Result<(), A::Error> {
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom("permissions must be empty"));
            }
            Ok(())
        }
    }
    deserializer.deserialize_seq(Empty)
}
fn script_field<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some) // Absence allowed; explicit null is not a source path.
}

#[derive(Debug)]
pub struct Manifest {
    id: String,
    name: String,
    entry: RelativePath,
    script: Option<RelativePath>,
    preferred_fps: u16,
}
impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        use ErrorCode::*;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(diagnostic(BudgetExceeded));
        }
        std::str::from_utf8(bytes).map_err(|_| diagnostic(InvalidUtf8))?;
        let raw: RawManifest = serde_json::from_slice(bytes).map_err(|error| {
            let code = match error.classify() {
                serde_json::error::Category::Data => ManifestSchema,
                _ => ManifestSyntax,
            };
            let mut result = diagnostic(code);
            result.line_column = Some((error.line(), error.column()));
            result
        })?;
        let field_error = |code, field| {
            let mut error = diagnostic(code);
            error.field = Some(field);
            error
        };
        if raw.format_version != FORMAT_VERSION {
            return Err(field_error(UnsupportedVersion, "formatVersion"));
        }
        if raw.api_version != API_VERSION {
            return Err(field_error(UnsupportedVersion, "apiVersion"));
        }
        if raw.id.is_empty()
            || raw.id.len() > 128
            || !raw
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
            || !raw.id.as_bytes()[0].is_ascii_alphanumeric()
        {
            return Err(field_error(InvalidId, "id"));
        }
        if raw.name.is_empty()
            || raw.name.len() > 256
            || raw.name.trim().is_empty()
            || raw.name.chars().any(char::is_control)
        {
            return Err(field_error(InvalidName, "name"));
        }
        if !(1..=60).contains(&raw.preferred_fps) {
            return Err(field_error(InvalidFps, "preferredFps"));
        }
        let entry =
            RelativePath::parse(&raw.entry).map_err(|_| field_error(InvalidPath, "entry"))?;
        let script = raw
            .script
            .map(|path| RelativePath::parse(&path).map_err(|_| field_error(InvalidPath, "script")))
            .transpose()?;
        if !entry.as_str().ends_with(".html") {
            return Err(field_error(SourceType, "entry"));
        }
        if script
            .as_ref()
            .is_some_and(|path| !path.as_str().ends_with(".js"))
        {
            return Err(field_error(SourceType, "script"));
        }
        Ok(Self {
            id: raw.id,
            name: raw.name,
            entry,
            script,
            preferred_fps: raw.preferred_fps,
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn entry(&self) -> &RelativePath {
        &self.entry
    }
    pub fn script(&self) -> Option<&RelativePath> {
        self.script.as_ref()
    }
    pub fn preferred_fps(&self) -> u16 {
        self.preferred_fps
    }
}
