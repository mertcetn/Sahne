//! Experimental resolved scene data. No source parser, asset decoder or renderer.
use std::{collections::BTreeSet, fmt};
mod codec;
pub use codec::decode;
pub mod animation;
pub mod hierarchy;
pub mod timer;
pub mod transaction;

pub const SCENE_VERSION: u16 = 1;
pub const MAX_NODES: usize = 4096;
pub const MAX_RESOURCES: usize = 1024;
pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_TOTAL_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_SCENE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CANVAS_DIMENSION: u32 = 8192;
pub const MAX_CANVAS_PIXELS: u64 = 32 * 1024 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 4096;
pub const MAX_DECODED_IMAGE_BYTES: u64 = 128 * 1024 * 1024;

/// Straight (unpremultiplied) sRGB bytes; conversion/compositing belongs to rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);

/// Resolved canvas-relative pixels, with no hierarchy or transform yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Rectangle {
        color: Color,
    },
    Text {
        text: String,
        font_size: f32,
        color: Color,
    },
    Image {
        resource: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: u32,
    pub bounds: Rect,
    pub content: Content,
}

/// Untrusted dimension declaration, not decoded or authenticated image data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageResource {
    pub id: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub version: u16,
    pub width: u32,
    pub height: u32,
    pub background: Color,
    pub resources: Vec<ImageResource>,
    /// Back-to-front order. IDs are scene-local, not trusted live host handles.
    pub nodes: Vec<Node>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    UnsupportedVersion,
    InvalidEncoding,
    Truncated,
    BudgetExceeded,
    InvalidCanvas,
    InvalidResource,
    DuplicateResource,
    InvalidNode,
    DuplicateNode,
    UnknownResource,
    NonFinite,
    InvalidGeometry,
    InvalidText,
    InvalidHierarchy,
    InvalidEffect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Scene,
    Node(usize),
    Resource(usize),
}

/// Bounded structured diagnostics: never interpolate untrusted source/text/path data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneError {
    pub code: ErrorCode,
    pub context: Context,
}
impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Scene {:?} at {:?}", self.code, self.context)
    }
}
impl std::error::Error for SceneError {}
type Result<T> = std::result::Result<T, SceneError>;
fn error(code: ErrorCode, context: Context) -> SceneError {
    SceneError { code, context }
}

/// Immutable validated value; decoding in another process must validate independently.
#[derive(Debug, PartialEq)]
pub struct ValidatedScene(Scene);
impl ValidatedScene {
    pub fn scene(&self) -> &Scene {
        &self.0
    }
    pub fn encode(&self) -> Vec<u8> {
        codec::encode(&self.0)
    }
}

impl Scene {
    pub fn validate(self) -> Result<ValidatedScene> {
        use ErrorCode::*;
        if self.version != SCENE_VERSION {
            return Err(error(UnsupportedVersion, Context::Scene));
        }
        if self.width == 0
            || self.height == 0
            || self.width > MAX_CANVAS_DIMENSION
            || self.height > MAX_CANVAS_DIMENSION
            || u64::from(self.width) * u64::from(self.height) > MAX_CANVAS_PIXELS
        {
            return Err(error(InvalidCanvas, Context::Scene));
        }
        if self.nodes.len() > MAX_NODES || self.resources.len() > MAX_RESOURCES {
            return Err(error(BudgetExceeded, Context::Scene));
        }
        let mut resources = BTreeSet::new();
        let mut decoded = 0u64;
        for (index, resource) in self.resources.iter().enumerate() {
            let context = Context::Resource(index);
            if resource.id == 0
                || resource.width == 0
                || resource.height == 0
                || resource.width > MAX_IMAGE_DIMENSION
                || resource.height > MAX_IMAGE_DIMENSION
            {
                return Err(error(InvalidResource, context));
            }
            if !resources.insert(resource.id) {
                return Err(error(DuplicateResource, context));
            }
            // Dimensions are bounded before multiplication; include unused declarations.
            decoded = decoded
                .checked_add(u64::from(resource.width) * u64::from(resource.height) * 4)
                .ok_or(error(BudgetExceeded, context))?;
            if decoded > MAX_DECODED_IMAGE_BYTES {
                return Err(error(BudgetExceeded, context));
            }
        }
        let mut ids = BTreeSet::new();
        let mut text_bytes = 0usize;
        for (index, node) in self.nodes.iter().enumerate() {
            let context = Context::Node(index);
            if node.id == 0 {
                return Err(error(InvalidNode, context));
            }
            if !ids.insert(node.id) {
                return Err(error(DuplicateNode, context));
            }
            let Rect {
                x,
                y,
                width,
                height,
            } = node.bounds;
            if [x, y, width, height].iter().any(|v| !v.is_finite()) {
                return Err(error(NonFinite, context));
            }
            if x.abs() > 32768.0
                || y.abs() > 32768.0
                || !(0.0..=32768.0).contains(&width)
                || !(0.0..=32768.0).contains(&height)
            {
                return Err(error(InvalidGeometry, context));
            }
            match &node.content {
                Content::Rectangle { .. } => {}
                Content::Image { resource } if !resources.contains(resource) => {
                    return Err(error(UnknownResource, context));
                }
                Content::Image { .. } => {}
                Content::Text {
                    text, font_size, ..
                } => {
                    if !font_size.is_finite() {
                        return Err(error(NonFinite, context));
                    }
                    text_bytes = text_bytes
                        .checked_add(text.len())
                        .ok_or(error(BudgetExceeded, context))?;
                    if text.len() > MAX_TEXT_BYTES || text_bytes > MAX_TOTAL_TEXT_BYTES {
                        return Err(error(BudgetExceeded, context));
                    }
                    if !(1.0..=512.0).contains(font_size)
                        || text.chars().any(|c| c.is_control() && c != '\n')
                    {
                        return Err(error(InvalidText, context));
                    }
                }
            }
        }
        Ok(ValidatedScene(self))
    }
}

#[cfg(test)]
mod tests;
