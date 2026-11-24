//! CPU-only admission and ordered pass planning. No device, window or font access.
use crate::Result;
use wallpaper_scene::{
    Color, Content, Rect,
    hierarchy::{Command, LayerContent, ValidatedHierarchy},
};

pub const MAX_TEXTURE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_TEXT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_TEXT_SURFACES: usize = 128;
pub const MAX_TEXT_DIMENSION: u32 = 4096;
pub const MAX_RASTER_FONT: f32 = 512.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Texture {
    Image(u32),
    Target(usize),
    Text(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    Rectangle,
    Straight(Texture),
    Premultiplied(Texture),
}
#[derive(Clone, Debug)]
pub struct Quad {
    pub bounds: Rect,
    pub clip: Rect,
    pub color: [f32; 4],
    pub opacity: f32,
    pub uv: [f32; 2],
}
#[derive(Clone, Copy, Debug)]
pub struct Op {
    pub target: usize,
    pub quad: usize,
    pub paint: Paint,
}
#[derive(Debug)]
pub struct Text {
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    pub width: f32,
    pub height: f32,
    pub raster_scale: f32,
    pub pixels: [u32; 2],
}
pub struct Plan {
    pub width: u32,
    pub height: u32,
    pub background: Color,
    pub targets: usize,
    pub quads: Vec<Quad>,
    pub ops: Vec<Op>,
    pub texts: Vec<Text>,
    pub texture_bytes: u64,
}
pub fn linear_color(color: [u8; 4]) -> [f32; 4] {
    let channel = |byte: u8| {
        let v = f32::from(byte) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    [
        channel(color[0]),
        channel(color[1]),
        channel(color[2]),
        f32::from(color[3]) / 255.0,
    ]
}
pub fn fit(canvas: [u32; 2], surface: [u32; 2]) -> Rect {
    let scale = (surface[0] as f32 / canvas[0] as f32).min(surface[1] as f32 / canvas[1] as f32);
    let width = canvas[0] as f32 * scale;
    let height = canvas[1] as f32 * scale;
    Rect {
        x: (surface[0] as f32 - width) * 0.5,
        y: (surface[1] as f32 - height) * 0.5,
        width,
        height,
    }
}
/// Coarse integer scissor encloses fractional clipping; the shader tests pixel centers exactly.
pub fn scissor(clip: Rect, width: u32, height: u32) -> Option<[u32; 4]> {
    let x = clip.x.floor().clamp(0.0, width as f32) as u32;
    let y = clip.y.floor().clamp(0.0, height as f32) as u32;
    let right = (clip.x + clip.width).ceil().clamp(0.0, width as f32) as u32;
    let bottom = (clip.y + clip.height).ceil().clamp(0.0, height as f32) as u32;
    (clip.width > 0.0 && clip.height > 0.0 && right > x && bottom > y).then_some([
        x,
        y,
        right.saturating_sub(x),
        bottom.saturating_sub(y),
    ])
}
fn visible(a: Rect, b: Rect) -> bool {
    a.width > 0.0
        && a.height > 0.0
        && b.width > 0.0
        && b.height > 0.0
        && a.x < b.x + b.width
        && b.x < a.x + a.width
        && a.y < b.y + b.height
        && b.y < a.y + a.height
}
fn bytes(width: u32, height: u32, max: u32) -> Result<u64> {
    if width == 0 || height == 0 || width > max || height > max {
        return Err("Group texture dimensions exceed policy/device limits".into());
    }
    Ok(u64::from(width) * u64::from(height) * 4)
}
impl Plan {
    pub fn new(scene: &ValidatedHierarchy, max_dimension: u32) -> Result<Self> {
        let data = scene.scene();
        let canvas = Rect {
            x: 0.0,
            y: 0.0,
            width: data.width as f32,
            height: data.height as f32,
        };
        let target_bytes = bytes(data.width, data.height, max_dimension)?;
        let mut total = target_bytes;
        for resource in &data.resources {
            total = total
                .checked_add(bytes(resource.width, resource.height, max_dimension)?)
                .ok_or("Group texture budget overflow")?;
        }
        let mut plan = Self {
            width: data.width,
            height: data.height,
            background: data.background,
            targets: 1,
            quads: vec![],
            ops: vec![],
            texts: vec![],
            texture_bytes: 0,
        };
        let mut current = 0;
        let mut stack = vec![];
        let mut text_bytes = 0u64;
        for command in scene.commands() {
            match *command {
                Command::BeginGroup(index) => {
                    let LayerContent::Group { opacity, .. } = data.layers[index].content else {
                        return Err("Invalid validated group command".into());
                    };
                    let intermediate = if opacity != 1.0 {
                        let target = plan.targets;
                        plan.targets += 1;
                        total = total
                            .checked_add(target_bytes)
                            .ok_or("Group texture budget overflow")?;
                        Some(target)
                    } else {
                        None
                    };
                    stack.push((index, current, intermediate));
                    if let Some(target) = intermediate {
                        current = target;
                    }
                }
                Command::EndGroup(index) => {
                    let (open, parent, intermediate) =
                        stack.pop().ok_or("Unbalanced group commands")?;
                    if open != index {
                        return Err("Mismatched group command".into());
                    }
                    if let Some(target) = intermediate {
                        let LayerContent::Group { opacity, .. } = data.layers[index].content else {
                            unreachable!()
                        };
                        let quad = Quad {
                            bounds: canvas,
                            clip: scene.resolved()[index].clip,
                            color: [1.0; 4],
                            opacity,
                            uv: [1.0; 2],
                        };
                        plan.push(parent, quad, Paint::Premultiplied(Texture::Target(target)));
                    }
                    current = parent;
                }
                Command::Draw(index) => {
                    let layer = &data.layers[index];
                    let resolved = scene.resolved()[index];
                    if !visible(resolved.bounds, resolved.clip) {
                        continue;
                    }
                    let mut quad = Quad {
                        bounds: resolved.bounds,
                        clip: resolved.clip,
                        color: [1.0; 4],
                        opacity: 1.0,
                        uv: [1.0; 2],
                    };
                    let paint = match &layer.content {
                        LayerContent::Draw(Content::Rectangle { color }) => {
                            quad.color = linear_color(color.0);
                            Paint::Rectangle
                        }
                        LayerContent::Draw(Content::Image { resource }) => {
                            Paint::Straight(Texture::Image(*resource))
                        }
                        LayerContent::Draw(Content::Text {
                            text,
                            font_size,
                            color,
                        }) => {
                            let raster_scale = resolved.transform.scale[0]
                                .max(resolved.transform.scale[1])
                                .max(1.0);
                            if *font_size * raster_scale > MAX_RASTER_FONT
                                || plan.texts.len() >= MAX_TEXT_SURFACES
                            {
                                return Err("Group text raster budget exceeded".into());
                            }
                            let w = (layer.bounds.width * raster_scale).ceil();
                            let h = (layer.bounds.height * raster_scale).ceil();
                            if w > MAX_TEXT_DIMENSION as f32 || h > MAX_TEXT_DIMENSION as f32 {
                                return Err(
                                    "Group text surface exceeds raster dimension limit".into()
                                );
                            }
                            let pixels = [w as u32, h as u32];
                            let size = bytes(pixels[0], pixels[1], max_dimension)?;
                            text_bytes = text_bytes
                                .checked_add(size)
                                .ok_or("Text raster budget overflow")?;
                            if text_bytes > MAX_TEXT_BYTES {
                                return Err("Group text surface budget exceeded".into());
                            }
                            total = total
                                .checked_add(size)
                                .ok_or("Group texture budget overflow")?;
                            quad.uv = [
                                layer.bounds.width * raster_scale / w,
                                layer.bounds.height * raster_scale / h,
                            ];
                            let id = plan.texts.len();
                            plan.texts.push(Text {
                                text: text.clone(),
                                font_size: *font_size,
                                color: *color,
                                width: layer.bounds.width,
                                height: layer.bounds.height,
                                raster_scale,
                                pixels,
                            });
                            Paint::Premultiplied(Texture::Text(id))
                        }
                        _ => return Err("Unexpected group in draw command".into()),
                    };
                    plan.push(current, quad, paint);
                }
            }
            if total > MAX_TEXTURE_BYTES {
                return Err("Group total texture budget exceeded".into());
            }
        }
        if !stack.is_empty() {
            return Err("Unclosed group commands".into());
        }
        if total > MAX_TEXTURE_BYTES {
            return Err("Group total texture budget exceeded".into());
        }
        plan.texture_bytes = total;
        Ok(plan)
    }
    fn push(&mut self, target: usize, quad: Quad, paint: Paint) {
        if scissor(quad.clip, self.width, self.height).is_none() {
            return;
        }
        let index = self.quads.len();
        self.quads.push(quad);
        self.ops.push(Op {
            target,
            quad: index,
            paint,
        });
    }
}
