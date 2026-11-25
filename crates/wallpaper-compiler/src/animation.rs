//! Explicit typed animation author-ID association; no CSS animation parser.
use crate::*;
use std::collections::BTreeMap;
use wallpaper_scene::{
    animation::{self, Animation, Iterations, Key, Property, Track, ValidatedAnimation},
    hierarchy::ValidatedHierarchy,
};

#[derive(Clone, Debug)]
pub struct AuthorTrack {
    pub target: String,
    pub property: Property,
    pub keys: Vec<Key>,
}
#[derive(Clone, Debug)]
pub struct Spec {
    pub duration_ms: u32,
    pub delay_ms: u32,
    pub iterations: Iterations,
    pub alternate: bool,
    pub tracks: Vec<AuthorTrack>,
}
#[derive(Debug)]
pub enum Error {
    Source(Diagnostic),
    Target(usize),
    Animation(animation::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(e) => e.fmt(f),
            Self::Target(i) => write!(f, "Invalid animation author target at track {i}"),
            Self::Animation(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub struct CompiledAnimation {
    animation: ValidatedAnimation,
    symbols: BTreeMap<String, u32>,
    images: BTreeMap<u32, wallpaper_media::RgbaImage>,
    script: Option<RelativePath>,
}
impl CompiledAnimation {
    pub fn animation(&self) -> &ValidatedAnimation {
        &self.animation
    }
    pub fn symbols(&self) -> &BTreeMap<String, u32> {
        &self.symbols
    }
    pub fn images(&self) -> &BTreeMap<u32, wallpaper_media::RgbaImage> {
        &self.images
    }
    pub fn script(&self) -> Option<&RelativePath> {
        self.script.as_ref()
    }
}
pub(crate) fn bind(
    base: ValidatedHierarchy,
    symbols: BTreeMap<String, u32>,
    script: Option<RelativePath>,
    images: BTreeMap<u32, wallpaper_media::RgbaImage>,
    spec: Spec,
) -> Result<CompiledAnimation> {
    if spec.tracks.is_empty() || spec.tracks.len() > animation::MAX_TRACKS {
        return Err(Error::Animation(animation::Error {
            code: animation::Code::Budget,
            track: None,
        }));
    }
    let mut tracks = Vec::with_capacity(spec.tracks.len());
    for (i, track) in spec.tracks.into_iter().enumerate() {
        if !identifier(&track.target) {
            return Err(Error::Target(i));
        }
        let target = *symbols.get(&track.target).ok_or(Error::Target(i))?;
        tracks.push(Track {
            target,
            property: track.property,
            keys: track.keys,
        });
    }
    let animation = Animation {
        version: animation::VERSION,
        duration_ms: spec.duration_ms,
        delay_ms: spec.delay_ms,
        iterations: spec.iterations,
        alternate: spec.alternate,
        tracks,
    }
    .validate(base)
    .map_err(Error::Animation)?;
    Ok(CompiledAnimation {
        animation,
        symbols,
        images,
        script,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec(target: &str) -> Spec {
        Spec {
            duration_ms: 1000,
            delay_ms: 0,
            iterations: Iterations::Count(1),
            alternate: false,
            tracks: vec![AuthorTrack {
                target: target.into(),
                property: Property::TranslateX,
                keys: vec![
                    Key {
                        time_ms: 0,
                        value: 0.0,
                    },
                    Key {
                        time_ms: 1000,
                        value: 100.0,
                    },
                ],
            }],
        }
    }
    fn viewport() -> Viewport {
        Viewport {
            width: 1920,
            height: 1080,
            background: wallpaper_scene::Color([0, 0, 0, 255]),
        }
    }
    #[test]
    fn author_binding_roundtrips_independently_without_losing_assets() {
        let compiled = compile_animated(
            &fixtures::effect_groups_package().unwrap(),
            viewport(),
            spec("outer"),
        )
        .unwrap();
        assert_eq!(
            compiled.animation().animation().tracks[0].target,
            compiled.symbols()["outer"]
        );
        let decoded = animation::decode(&compiled.animation().encode()).unwrap();
        wallpaper_media::validate_hierarchy_assets(decoded.base(), compiled.images()).unwrap();
        let sample = decoded
            .sample(std::time::Duration::from_millis(500))
            .unwrap();
        assert_eq!(
            sample.scene.resolved()[compiled.symbols()["outer"] as usize - 1]
                .bounds
                .x,
            130.0
        );
        assert_eq!(compiled.images().len(), 1);
        assert!(compiled.script().is_none());
    }
    #[test]
    fn unknown_invalid_duplicate_targets_and_invalid_keys_fail() {
        let package = fixtures::effect_groups_package().unwrap();
        for target in ["missing", "nestedRed child", "", &"x".repeat(65)] {
            assert!(matches!(
                compile_animated(&package, viewport(), spec(target)),
                Err(Error::Target(0))
            ));
        }
        let mut duplicate = spec("outer");
        duplicate.tracks.push(duplicate.tracks[0].clone());
        assert!(matches!(
            compile_animated(&package, viewport(), duplicate),
            Err(Error::Animation(animation::Error {
                code: animation::Code::Duplicate,
                ..
            }))
        ));
        let mut invalid = spec("outer");
        invalid.tracks[0].keys[1].value = f32::NAN;
        assert!(matches!(
            compile_animated(&package, viewport(), invalid),
            Err(Error::Animation(animation::Error {
                code: animation::Code::Value,
                ..
            }))
        ));
    }
}
