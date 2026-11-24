//! Experimental version 2: ordered groups are preserved, never flattened to leaf alpha.
use super::*;
mod wire;
pub use wire::decode;

pub const VERSION: u16 = 2;
pub const MAX_DEPTH: usize = 32;
pub const MAX_OPACITY_GROUPS: usize = 8;
pub const MAX_COMPOSITING_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_WORLD_COORDINATE: f32 = 1_048_576.0;
pub const MAX_WORLD_SCALE: f32 = 64.0;

/// Axis-aligned local transform, around the group's top-left origin.
/// p_parent = bounds.origin + translate + scale * p_local.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translate: [f32; 2],
    pub scale: [f32; 2],
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            translate: [0.0; 2],
            scale: [1.0; 2],
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum LayerContent {
    Group {
        transform: Transform,
        opacity: f32,
        clip: bool,
    },
    Draw(Content),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: u32,
    /// Zero is the implicit canvas root; other parents must be open groups.
    pub parent: u32,
    pub bounds: Rect,
    pub content: LayerContent,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Hierarchy {
    pub version: u16,
    pub width: u32,
    pub height: u32,
    pub background: Color,
    pub resources: Vec<ImageResource>,
    /// Depth-first preorder. Each subtree is contiguous; siblings paint back to front.
    pub layers: Vec<Layer>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedLayer {
    /// Maps the layer's local origin to canvas pixels; opacity is NOT multiplied here.
    pub transform: Transform,
    pub bounds: Rect,
    /// Effective canvas/ancestor clip, including this group's own clip if enabled.
    pub clip: Rect,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    BeginGroup(usize),
    Draw(usize),
    EndGroup(usize),
}
#[derive(Debug, PartialEq)]
pub struct ValidatedHierarchy {
    data: Hierarchy,
    resolved: Vec<ResolvedLayer>,
    commands: Vec<Command>,
}
impl ValidatedHierarchy {
    pub fn scene(&self) -> &Hierarchy {
        &self.data
    }
    pub fn resolved(&self) -> &[ResolvedLayer] {
        &self.resolved
    }
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
    pub fn encode(&self) -> Vec<u8> {
        wire::encode(&self.data)
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect {
        x,
        y,
        width: ((a.x + a.width).min(b.x + b.width) - x).max(0.0),
        height: ((a.y + a.height).min(b.y + b.height) - y).max(0.0),
    }
}

impl Hierarchy {
    pub fn validate(self) -> Result<ValidatedHierarchy> {
        use ErrorCode::*;
        if self.version != VERSION {
            return Err(error(UnsupportedVersion, Context::Scene));
        }
        if self.layers.len() > MAX_NODES || self.resources.len() > MAX_RESOURCES {
            return Err(error(BudgetExceeded, Context::Scene));
        }
        // Reuse the established v1 numeric/resource/text validation policy.
        // Bound text BEFORE cloning any primitive into this temporary validation view.
        let mut text_bytes = 0usize;
        for (index, layer) in self.layers.iter().enumerate() {
            if let LayerContent::Draw(Content::Text { text, .. }) = &layer.content {
                text_bytes = text_bytes
                    .checked_add(text.len())
                    .ok_or(error(BudgetExceeded, Context::Node(index)))?;
                if text.len() > MAX_TEXT_BYTES || text_bytes > MAX_TOTAL_TEXT_BYTES {
                    return Err(error(BudgetExceeded, Context::Node(index)));
                }
            }
        }
        let view = Scene {
            version: SCENE_VERSION,
            width: self.width,
            height: self.height,
            background: self.background,
            resources: self.resources.clone(),
            nodes: self
                .layers
                .iter()
                .map(|layer| Node {
                    id: layer.id,
                    bounds: layer.bounds,
                    content: match &layer.content {
                        LayerContent::Draw(content) => content.clone(),
                        LayerContent::Group { .. } => Content::Rectangle {
                            color: Color([0; 4]),
                        },
                    },
                })
                .collect(),
        };
        view.validate()?;
        let canvas = Rect {
            x: 0.0,
            y: 0.0,
            width: self.width as f32,
            height: self.height as f32,
        };
        let mut stack: Vec<usize> = vec![];
        let mut resolved: Vec<ResolvedLayer> = Vec::with_capacity(self.layers.len());
        let mut commands = Vec::with_capacity(self.layers.len() * 2);
        let mut opacity_groups = 0usize;
        for (index, layer) in self.layers.iter().enumerate() {
            let context = Context::Node(index);
            while stack
                .last()
                .is_some_and(|&i| self.layers[i].id != layer.parent)
            {
                commands.push(Command::EndGroup(stack.pop().unwrap()));
            }
            if layer.parent != 0 && stack.is_empty() {
                return Err(error(InvalidHierarchy, context));
            }
            let (parent, parent_clip) = stack.last().map_or((Transform::default(), canvas), |&i| {
                (resolved[i].transform, resolved[i].clip)
            });
            let mut world = parent;
            world.translate = [
                parent.translate[0] + parent.scale[0] * layer.bounds.x,
                parent.translate[1] + parent.scale[1] * layer.bounds.y,
            ];
            let group = match &layer.content {
                LayerContent::Group {
                    transform,
                    opacity,
                    clip,
                } => {
                    if transform
                        .translate
                        .iter()
                        .chain(&transform.scale)
                        .chain(std::iter::once(opacity))
                        .any(|v| !v.is_finite())
                    {
                        return Err(error(NonFinite, context));
                    }
                    if transform.translate.iter().any(|v| v.abs() > 32768.0)
                        || transform.scale.iter().any(|v| !(0.0..=16.0).contains(v))
                        || !(0.0..=1.0).contains(opacity)
                    {
                        return Err(error(InvalidEffect, context));
                    }
                    if stack.len() >= MAX_DEPTH {
                        return Err(error(BudgetExceeded, context));
                    }
                    if *opacity != 1.0 {
                        opacity_groups += 1;
                        let bytes = u64::from(self.width)
                            * u64::from(self.height)
                            * 4
                            * opacity_groups as u64;
                        if opacity_groups > MAX_OPACITY_GROUPS || bytes > MAX_COMPOSITING_BYTES {
                            return Err(error(BudgetExceeded, context));
                        }
                    }
                    for axis in 0..2 {
                        world.translate[axis] += parent.scale[axis] * transform.translate[axis];
                        world.scale[axis] *= transform.scale[axis];
                    }
                    Some(*clip)
                }
                LayerContent::Draw(_) => None,
            };
            let bounds = Rect {
                x: world.translate[0],
                y: world.translate[1],
                width: layer.bounds.width * world.scale[0],
                height: layer.bounds.height * world.scale[1],
            };
            if world
                .scale
                .iter()
                .any(|v| !v.is_finite() || *v > MAX_WORLD_SCALE)
                || [
                    bounds.x,
                    bounds.y,
                    bounds.x + bounds.width,
                    bounds.y + bounds.height,
                ]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > MAX_WORLD_COORDINATE)
            {
                return Err(error(InvalidEffect, context));
            }
            resolved.push(ResolvedLayer {
                transform: world,
                bounds,
                clip: if group == Some(true) {
                    intersect(parent_clip, bounds)
                } else {
                    parent_clip
                },
            });
            if group.is_some() {
                commands.push(Command::BeginGroup(index));
                stack.push(index);
            } else {
                commands.push(Command::Draw(index));
            }
        }
        while let Some(index) = stack.pop() {
            commands.push(Command::EndGroup(index));
        }
        Ok(ValidatedHierarchy {
            data: self,
            resolved,
            commands,
        })
    }
}

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
#[cfg(test)]
mod tests;
