//! Experimental typed native animation, bound to one independently validated v2 scene.
//! No CSS parser, script callbacks, renderer, timer thread or external input API.
use crate::hierarchy::{self, LayerContent, ValidatedHierarchy};
use std::{
    collections::BTreeMap,
    fmt,
    time::{Duration, Instant},
};

pub const VERSION: u16 = 1;
pub const MAX_TRACKS: usize = 128;
pub const MAX_KEYS_PER_TRACK: usize = 64;
pub const MAX_KEYS: usize = 2048;
pub const MAX_DURATION_MS: u32 = 86_400_000;
pub const MAX_ITERATIONS: u32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Property {
    TranslateX,
    TranslateY,
    ScaleX,
    ScaleY,
    Opacity,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub time_ms: u32,
    pub value: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub target: u32,
    pub property: Property,
    pub keys: Vec<Key>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Iterations {
    Count(u32),
    Forever,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub version: u16,
    pub duration_ms: u32,
    pub delay_ms: u32,
    pub iterations: Iterations,
    pub alternate: bool,
    pub tracks: Vec<Track>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Version,
    Budget,
    Timing,
    Target,
    Duplicate,
    KeyTime,
    Value,
    Envelope,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: Code,
    pub track: Option<usize>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Animation {:?} at track {:?}", self.code, self.track)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn error(code: Code, track: Option<usize>) -> Error {
    Error { code, track }
}

/// Owns its base scene; an unrelated later scene cannot reuse these target bindings.
#[derive(Debug)]
pub struct ValidatedAnimation {
    base: ValidatedHierarchy,
    data: Animation,
    indices: Vec<usize>,
}
pub struct Sample {
    pub scene: ValidatedHierarchy,
    /// Includes the delay phase; false only after a finite run has filled its final pose.
    pub active: bool,
}
impl Animation {
    pub fn validate(self, base: ValidatedHierarchy) -> Result<ValidatedAnimation> {
        if self.version != VERSION {
            return Err(error(Code::Version, None));
        }
        if self.duration_ms == 0
            || self.duration_ms > MAX_DURATION_MS
            || self.delay_ms > MAX_DURATION_MS
            || matches!(self.iterations, Iterations::Count(n) if n == 0 || n > MAX_ITERATIONS)
        {
            return Err(error(Code::Timing, None));
        }
        if self.tracks.is_empty() || self.tracks.len() > MAX_TRACKS {
            return Err(error(Code::Budget, None));
        }
        let layers = &base.scene().layers;
        let targets: BTreeMap<_, _> = layers.iter().enumerate().map(|(i, l)| (l.id, i)).collect();
        let mut ranges = BTreeMap::new();
        let mut indices = Vec::with_capacity(self.tracks.len());
        let mut keys = 0usize;
        for (i, track) in self.tracks.iter().enumerate() {
            let at = Some(i);
            keys = keys
                .checked_add(track.keys.len())
                .ok_or(error(Code::Budget, at))?;
            if !(2..=MAX_KEYS_PER_TRACK).contains(&track.keys.len()) || keys > MAX_KEYS {
                return Err(error(Code::Budget, at));
            }
            let index = *targets.get(&track.target).ok_or(error(Code::Target, at))?;
            if !matches!(layers[index].content, LayerContent::Group { .. }) {
                return Err(error(Code::Target, at));
            }
            if track.keys.first().unwrap().time_ms != 0
                || track.keys.last().unwrap().time_ms != self.duration_ms
                || track.keys.windows(2).any(|p| p[0].time_ms >= p[1].time_ms)
            {
                return Err(error(Code::KeyTime, at));
            }
            let mut range = Range(f64::INFINITY, f64::NEG_INFINITY);
            for key in &track.keys {
                let valid = key.value.is_finite()
                    && match track.property {
                        Property::TranslateX | Property::TranslateY => key.value.abs() <= 32768.0,
                        Property::ScaleX | Property::ScaleY => (0.0..=16.0).contains(&key.value),
                        Property::Opacity => (0.0..=1.0).contains(&key.value),
                    };
                if !valid {
                    return Err(error(Code::Value, at));
                }
                range.0 = range.0.min(f64::from(key.value));
                range.1 = range.1.max(f64::from(key.value));
            }
            if ranges.insert((index, track.property), range).is_some() {
                return Err(error(Code::Duplicate, at));
            }
            indices.push(index);
        }
        validate_envelope(&base, &ranges)?;
        Ok(ValidatedAnimation {
            base,
            data: self,
            indices,
        })
    }
}

// Conservative intervals include every independently moving parent/child, not just
// matching key timestamps. Positive scales make interval multiplication explicit.
#[derive(Clone, Copy)]
struct Range(f64, f64);
impl Range {
    fn point(x: f32) -> Self {
        Self(f64::from(x), f64::from(x))
    }
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1)
    }
    fn mul(self, other: Self) -> Self {
        let values = [
            self.0 * other.0,
            self.0 * other.1,
            self.1 * other.0,
            self.1 * other.1,
        ];
        Self(
            values.iter().copied().fold(f64::INFINITY, f64::min),
            values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    }
    fn bounded(self, limit: f64) -> bool {
        self.0.is_finite() && self.1.is_finite() && self.0 >= -limit && self.1 <= limit
    }
}
fn validate_envelope(
    base: &ValidatedHierarchy,
    ranges: &BTreeMap<(usize, Property), Range>,
) -> Result<()> {
    let mut worlds: BTreeMap<u32, ([Range; 2], [Range; 2])> = BTreeMap::new();
    let mut opacity_groups = 0u64;
    for (index, layer) in base.scene().layers.iter().enumerate() {
        let (parent_pos, parent_scale) = if layer.parent == 0 {
            ([Range::point(0.0); 2], [Range::point(1.0); 2])
        } else {
            worlds[&layer.parent]
        };
        let mut position = [Range::point(layer.bounds.x), Range::point(layer.bounds.y)];
        let mut scale = [Range::point(1.0); 2];
        if let LayerContent::Group {
            transform, opacity, ..
        } = layer.content
        {
            for (axis, (translation, scaling)) in [
                (Property::TranslateX, Property::ScaleX),
                (Property::TranslateY, Property::ScaleY),
            ]
            .into_iter()
            .enumerate()
            {
                position[axis] = position[axis].add(
                    *ranges
                        .get(&(index, translation))
                        .unwrap_or(&Range::point(transform.translate[axis])),
                );
                scale[axis] = *ranges
                    .get(&(index, scaling))
                    .unwrap_or(&Range::point(transform.scale[axis]));
            }
            // Reserve an intermediate for every animated opacity, even at value 1.
            if opacity != 1.0 || ranges.contains_key(&(index, Property::Opacity)) {
                opacity_groups += 1;
            }
        }
        for axis in 0..2 {
            position[axis] = parent_pos[axis].add(parent_scale[axis].mul(position[axis]));
            scale[axis] = parent_scale[axis].mul(scale[axis]);
            let size = if axis == 0 {
                layer.bounds.width
            } else {
                layer.bounds.height
            };
            let edge = position[axis].add(scale[axis].mul(Range::point(size)));
            if !position[axis].bounded(f64::from(hierarchy::MAX_WORLD_COORDINATE))
                || !edge.bounded(f64::from(hierarchy::MAX_WORLD_COORDINATE))
                || !scale[axis].bounded(f64::from(hierarchy::MAX_WORLD_SCALE))
            {
                return Err(error(Code::Envelope, None));
            }
        }
        worlds.insert(layer.id, (position, scale));
    }
    let bytes = u64::from(base.scene().width) * u64::from(base.scene().height) * 4 * opacity_groups;
    if opacity_groups > hierarchy::MAX_OPACITY_GROUPS as u64
        || bytes > hierarchy::MAX_COMPOSITING_BYTES
    {
        return Err(error(Code::Budget, None));
    }
    Ok(())
}

impl ValidatedAnimation {
    pub fn animation(&self) -> &Animation {
        &self.data
    }
    pub fn base(&self) -> &ValidatedHierarchy {
        &self.base
    }
    /// Absolute elapsed time; skips missed frames rather than iterating catch-up steps.
    /// The initial/final poses fill before delay/after finite completion respectively.
    pub fn sample(&self, elapsed: Duration) -> Result<Sample> {
        let elapsed_ns = elapsed.as_nanos();
        let delay = u128::from(self.data.delay_ms) * 1_000_000;
        let duration = u128::from(self.data.duration_ms) * 1_000_000;
        let time = elapsed_ns.saturating_sub(delay);
        let (iteration, mut within, active) = match self.data.iterations {
            Iterations::Count(n) if time >= duration * u128::from(n) => {
                (u128::from(n - 1), duration, false)
            }
            _ => (time / duration, time % duration, true),
        };
        if self.data.alternate && iteration % 2 != 0 {
            within = duration - within;
        }
        let time_ms = within as f64 / 1_000_000.0;
        let mut scene = self.base.scene().clone();
        for (track, &index) in self.data.tracks.iter().zip(&self.indices) {
            let value = interpolate(&track.keys, time_ms);
            let LayerContent::Group {
                transform, opacity, ..
            } = &mut scene.layers[index].content
            else {
                unreachable!("Validated group binding")
            };
            match track.property {
                Property::TranslateX => transform.translate[0] = value,
                Property::TranslateY => transform.translate[1] = value,
                Property::ScaleX => transform.scale[0] = value,
                Property::ScaleY => transform.scale[1] = value,
                Property::Opacity => *opacity = value,
            }
        }
        let scene = scene.validate().map_err(|_| error(Code::Envelope, None))?;
        Ok(Sample { scene, active })
    }
}
fn interpolate(keys: &[Key], time: f64) -> f32 {
    let upper = keys.partition_point(|k| f64::from(k.time_ms) <= time);
    if upper == 0 {
        return keys[0].value;
    }
    if upper == keys.len() {
        return keys[upper - 1].value;
    }
    let a = keys[upper - 1];
    let b = keys[upper];
    let fraction = (time - f64::from(a.time_ms)) / f64::from(b.time_ms - a.time_ms);
    (f64::from(a.value) * (1.0 - fraction) + f64::from(b.value) * fraction) as f32
}

/// Caller-owned monotonic clock. No sleeps, OS subscriptions or implicit pause policy.
#[derive(Debug)]
pub struct Clock {
    last: Instant,
    elapsed: Duration,
    paused: bool,
}
impl Clock {
    pub fn new(now: Instant) -> Self {
        Self {
            last: now,
            elapsed: Duration::ZERO,
            paused: false,
        }
    }
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn advance(&mut self, now: Instant) -> Result<Duration> {
        let delta = now
            .checked_duration_since(self.last)
            .ok_or(error(Code::Timing, None))?;
        let elapsed = if self.paused {
            self.elapsed
        } else {
            self.elapsed
                .checked_add(delta)
                .ok_or(error(Code::Timing, None))?
        };
        self.last = now;
        self.elapsed = elapsed;
        Ok(elapsed)
    }
    pub fn set_paused(&mut self, now: Instant, paused: bool) -> Result<Duration> {
        self.advance(now)?;
        self.paused = paused;
        Ok(self.elapsed)
    }
}

#[cfg(test)]
mod tests;
