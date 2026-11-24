use super::*;
pub(super) const MAGIC: &[u8; 8] = b"SAHNSCN\0";

pub(super) fn encode(scene: &Scene) -> Vec<u8> {
    // Validation bounds this estimate below MAX_SCENE_BYTES, including all text.
    let text_bytes: usize = scene
        .nodes
        .iter()
        .map(|node| match &node.content {
            Content::Text { text, .. } => text.len(),
            _ => 0,
        })
        .sum();
    let mut bytes =
        Vec::with_capacity(30 + scene.resources.len() * 12 + scene.nodes.len() * 33 + text_bytes);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&scene.version.to_le_bytes());
    put_u32(&mut bytes, scene.width);
    put_u32(&mut bytes, scene.height);
    bytes.extend_from_slice(&scene.background.0);
    put_u32(&mut bytes, scene.resources.len() as u32);
    put_u32(&mut bytes, scene.nodes.len() as u32);
    for resource in &scene.resources {
        for value in [resource.id, resource.width, resource.height] {
            put_u32(&mut bytes, value);
        }
    }
    for node in &scene.nodes {
        put_u32(&mut bytes, node.id);
        let Rect {
            x,
            y,
            width,
            height,
        } = node.bounds;
        for value in [x, y, width, height] {
            put_u32(&mut bytes, value.to_bits());
        }
        match &node.content {
            Content::Rectangle { color } => {
                bytes.push(1);
                bytes.extend_from_slice(&color.0);
            }
            Content::Text {
                text,
                font_size,
                color,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&color.0);
                put_u32(&mut bytes, font_size.to_bits());
                put_u32(&mut bytes, text.len() as u32);
                bytes.extend_from_slice(text.as_bytes());
            }
            Content::Image { resource } => {
                bytes.push(3);
                put_u32(&mut bytes, *resource);
            }
        }
    }
    bytes
}
pub(super) fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) struct Reader<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) position: usize,
    pub(super) context: Context,
    pub(super) text_bytes: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(error(ErrorCode::BudgetExceeded, self.context))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(error(ErrorCode::Truncated, self.context))?;
        self.position = end;
        Ok(bytes)
    }
    pub(super) fn u32(&mut self) -> Result<u32> {
        // take(4) guarantees this fixed-size conversion, including for hostile input.
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(super) fn float(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub(super) fn color(&mut self) -> Result<Color> {
        Ok(Color(self.take(4)?.try_into().unwrap()))
    }
    pub(super) fn text(&mut self) -> Result<String> {
        let count = self.u32()? as usize;
        if count > MAX_TEXT_BYTES {
            return Err(error(ErrorCode::BudgetExceeded, self.context));
        }
        self.text_bytes = self
            .text_bytes
            .checked_add(count)
            .ok_or(error(ErrorCode::BudgetExceeded, self.context))?;
        if self.text_bytes > MAX_TOTAL_TEXT_BYTES {
            return Err(error(ErrorCode::BudgetExceeded, self.context));
        }
        let context = self.context;
        let text = std::str::from_utf8(self.take(count)?)
            .map_err(|_| error(ErrorCode::InvalidText, context))?;
        Ok(text.to_owned())
    }
}

/// Parse bounded data and validate it independently. This performs no I/O or decoding
/// of image/font/source payloads; it is not the 64 KiB control-frame transport.
pub fn decode(bytes: &[u8]) -> Result<ValidatedScene> {
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
    if version != SCENE_VERSION {
        return Err(error(UnsupportedVersion, Context::Scene));
    }
    let width = reader.u32()?;
    let height = reader.u32()?;
    let background = reader.color()?;
    let resource_count = reader.u32()? as usize;
    let node_count = reader.u32()? as usize;
    if resource_count > MAX_RESOURCES || node_count > MAX_NODES {
        return Err(error(BudgetExceeded, Context::Scene));
    }
    // Even the shortest record must fit before count-driven Vec allocation.
    let minimum = resource_count * 12 + node_count * 25;
    if minimum > bytes.len() - reader.position {
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
    let mut nodes = Vec::with_capacity(node_count);
    for index in 0..node_count {
        reader.context = Context::Node(index);
        let id = reader.u32()?;
        let bounds = Rect {
            x: reader.float()?,
            y: reader.float()?,
            width: reader.float()?,
            height: reader.float()?,
        };
        let content = match reader.take(1)?[0] {
            1 => Content::Rectangle {
                color: reader.color()?,
            },
            2 => {
                let color = reader.color()?;
                let font_size = reader.float()?;
                Content::Text {
                    text: reader.text()?,
                    font_size,
                    color,
                }
            }
            3 => Content::Image {
                resource: reader.u32()?,
            },
            _ => return Err(error(InvalidEncoding, reader.context)),
        };
        nodes.push(Node {
            id,
            bounds,
            content,
        });
    }
    if reader.position != bytes.len() {
        return Err(error(InvalidEncoding, Context::Scene));
    }
    Scene {
        version,
        width,
        height,
        background,
        resources,
        nodes,
    }
    .validate()
}
