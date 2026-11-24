use super::*;
use crate::codec::{MAGIC, Reader, put_u32};

pub(super) fn encode(scene: &Hierarchy) -> Vec<u8> {
    let text: usize = scene
        .layers
        .iter()
        .map(|layer| match &layer.content {
            LayerContent::Draw(Content::Text { text, .. }) => text.len(),
            _ => 0,
        })
        .sum();
    let mut bytes =
        Vec::with_capacity(30 + scene.resources.len() * 12 + scene.layers.len() * 46 + text);
    bytes.extend(MAGIC);
    bytes.extend(scene.version.to_le_bytes());
    put_u32(&mut bytes, scene.width);
    put_u32(&mut bytes, scene.height);
    bytes.extend(scene.background.0);
    put_u32(&mut bytes, scene.resources.len() as u32);
    put_u32(&mut bytes, scene.layers.len() as u32);
    for resource in &scene.resources {
        for value in [resource.id, resource.width, resource.height] {
            put_u32(&mut bytes, value);
        }
    }
    for layer in &scene.layers {
        put_u32(&mut bytes, layer.id);
        put_u32(&mut bytes, layer.parent);
        for value in [
            layer.bounds.x,
            layer.bounds.y,
            layer.bounds.width,
            layer.bounds.height,
        ] {
            put_u32(&mut bytes, value.to_bits());
        }
        match &layer.content {
            LayerContent::Draw(Content::Rectangle { color }) => {
                bytes.push(1);
                bytes.extend(color.0);
            }
            LayerContent::Draw(Content::Text {
                text,
                font_size,
                color,
            }) => {
                bytes.push(2);
                bytes.extend(color.0);
                put_u32(&mut bytes, font_size.to_bits());
                put_u32(&mut bytes, text.len() as u32);
                bytes.extend(text.as_bytes());
            }
            LayerContent::Draw(Content::Image { resource }) => {
                bytes.push(3);
                put_u32(&mut bytes, *resource);
            }
            LayerContent::Group {
                transform,
                opacity,
                clip,
            } => {
                bytes.push(4);
                for value in transform
                    .translate
                    .iter()
                    .chain(&transform.scale)
                    .chain(std::iter::once(opacity))
                {
                    put_u32(&mut bytes, value.to_bits());
                }
                bytes.push(u8::from(*clip));
            }
        }
    }
    bytes
}

/// The explicit v2 decoder never accepts v1 or silently upgrades/flattens it.
pub fn decode(bytes: &[u8]) -> Result<ValidatedHierarchy> {
    use ErrorCode::*;
    if bytes.len() > MAX_SCENE_BYTES {
        return Err(error(BudgetExceeded, Context::Scene));
    }
    let mut reader = Reader {
        bytes,
        position: 0,
        context: Context::Scene,
        text_bytes: 0,
    };
    if reader.take(8)? != MAGIC {
        return Err(error(InvalidEncoding, Context::Scene));
    }
    let version = u16::from_le_bytes(reader.take(2)?.try_into().unwrap());
    if version != VERSION {
        return Err(error(UnsupportedVersion, Context::Scene));
    }
    let width = reader.u32()?;
    let height = reader.u32()?;
    let background = reader.color()?;
    let resource_count = reader.u32()? as usize;
    let count = reader.u32()? as usize;
    if resource_count > MAX_RESOURCES || count > MAX_NODES {
        return Err(error(BudgetExceeded, Context::Scene));
    }
    if resource_count * 12 + count * 29 > bytes.len() - reader.position {
        return Err(error(Truncated, Context::Scene));
    }
    let mut resources = Vec::with_capacity(resource_count);
    for index in 0..resource_count {
        reader.context = Context::Resource(index);
        resources.push(ImageResource {
            id: reader.u32()?,
            width: reader.u32()?,
            height: reader.u32()?,
        });
    }
    let mut layers = Vec::with_capacity(count);
    for index in 0..count {
        reader.context = Context::Node(index);
        let id = reader.u32()?;
        let parent = reader.u32()?;
        let bounds = Rect {
            x: reader.float()?,
            y: reader.float()?,
            width: reader.float()?,
            height: reader.float()?,
        };
        let content = match reader.take(1)?[0] {
            1 => LayerContent::Draw(Content::Rectangle {
                color: reader.color()?,
            }),
            2 => {
                let color = reader.color()?;
                let font_size = reader.float()?;
                LayerContent::Draw(Content::Text {
                    color,
                    font_size,
                    text: reader.text()?,
                })
            }
            3 => LayerContent::Draw(Content::Image {
                resource: reader.u32()?,
            }),
            4 => {
                let transform = Transform {
                    translate: [reader.float()?, reader.float()?],
                    scale: [reader.float()?, reader.float()?],
                };
                let opacity = reader.float()?;
                let clip = match reader.take(1)?[0] {
                    0 => false,
                    1 => true,
                    _ => return Err(error(InvalidEncoding, reader.context)),
                };
                LayerContent::Group {
                    transform,
                    opacity,
                    clip,
                }
            }
            _ => return Err(error(InvalidEncoding, reader.context)),
        };
        layers.push(Layer {
            id,
            parent,
            bounds,
            content,
        });
    }
    if reader.position != bytes.len() {
        return Err(error(InvalidEncoding, Context::Scene));
    }
    Hierarchy {
        version,
        width,
        height,
        background,
        resources,
        layers,
    }
    .validate()
}
