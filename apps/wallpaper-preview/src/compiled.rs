//! Renders the original compiled flat fixture only; no external scene/resource input.
use super::*;
use glyphon::Wrap;
use wallpaper_scene::{Content, ValidatedScene};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Example {
    Static,
    Image,
    Groups,
    GroupEffects,
    Animated,
    Clock,
}
impl Example {
    pub fn group_fixture(self) -> Option<groups::Fixture> {
        match self {
            Self::Groups => Some(groups::Fixture::Original),
            Self::GroupEffects => Some(groups::Fixture::Effects),
            Self::Animated => Some(groups::Fixture::Animated),
            _ => None,
        }
    }
}

struct TextItem {
    index: usize,
    buffer: Buffer,
    renderer: TextRenderer,
}
enum Item {
    Rectangle(u32),
    Text(Box<TextItem>),
    Image { offset: u32, resource: u32 },
}
pub struct Renderer {
    scene: Arc<ValidatedScene>,
    snapshot_key: Option<wallpaper_scene::transaction::SnapshotKey>,
    items: Vec<Item>,
    pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    images: std::collections::BTreeMap<u32, wgpu::BindGroup>,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    stride: usize,
}
impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        fonts: &mut FontSystem,
        atlas: &mut TextAtlas,
        example: Example,
    ) -> Result<Self> {
        if !format.is_srgb() {
            return Err("Compiled preview requires an sRGB surface".into());
        }
        let compiled = match example {
            Example::Clock => None,
            Example::Static | Example::Image => {
                let package = if example == Example::Static {
                    wallpaper_compiler::fixtures::static_package()?
                } else {
                    wallpaper_compiler::fixtures::image_package()?
                };
                Some(wallpaper_compiler::compile(
                    &package,
                    wallpaper_compiler::Viewport {
                        width: 1920,
                        height: 1080,
                        background: wallpaper_scene::Color([12, 17, 22, 255]),
                    },
                )?)
            }
            Example::Groups | Example::GroupEffects | Example::Animated => {
                return Err("Group example requires the version-2 renderer".into());
            }
        };
        let empty = std::collections::BTreeMap::new();
        let (scene, assets) = if let Some(compiled) = &compiled {
            (
                wallpaper_scene::decode(&compiled.scene().encode())?,
                compiled.images(),
            )
        } else {
            (clock::initial_scene()?, &empty)
        };
        wallpaper_media::validate_assets(&scene, assets)?;
        for image in assets.values() {
            if image.width() > device.limits().max_texture_dimension_2d
                || image.height() > device.limits().max_texture_dimension_2d
            {
                return Err("Image dimensions exceed device limits".into());
            }
        }
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        if alignment == 0 {
            return Err("Invalid uniform offset alignment".into());
        }
        let stride = 48usize.div_ceil(alignment) * alignment;
        let size = stride
            .checked_mul(scene.scene().nodes.len().max(1))
            .ok_or("Uniform buffer size overflow")?;
        if size as u64 > device.limits().max_buffer_size || size > u32::MAX as usize {
            return Err("Compiled uniform budget exceeds device limits".into());
        }
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Compiled node uniforms"),
            size: size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compiled node layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(48),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compiled node bindings"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: wgpu::BufferSize::new(48),
                }),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compiled layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compiled rectangle shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compiled.wgsl").into()),
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compiled image layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let image_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compiled image pipeline layout"),
                bind_group_layouts: &[Some(&layout), Some(&image_layout)],
                immediate_size: 0,
            });
        let image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compiled images"),
            layout: Some(&image_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_image"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Compiled image nearest clamp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let mut images = std::collections::BTreeMap::new();
        for (&id, image) in assets {
            let size = wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Validated compiled RGBA8 image"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image.pixels(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width() * 4),
                    rows_per_image: Some(image.height()),
                },
                size,
            );
            let view = texture.create_view(&Default::default());
            let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compiled image bindings"),
                layout: &image_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            images.insert(id, bindings);
        }
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compiled rectangles"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let mut items = vec![];
        for (index, node) in scene.scene().nodes.iter().enumerate() {
            match &node.content {
                Content::Rectangle { .. } => items.push(Item::Rectangle((index * stride) as u32)),
                Content::Text {
                    text, font_size, ..
                } => {
                    let mut buffer =
                        Buffer::new(fonts, Metrics::new(*font_size, *font_size * 1.25));
                    buffer.set_size(Some(node.bounds.width), Some(node.bounds.height));
                    buffer.set_wrap(Wrap::None);
                    buffer.set_text(
                        text,
                        &Attrs::new().family(Family::Name("Noto Sans")),
                        Shaping::Advanced,
                        None,
                    );
                    buffer.shape_until_scroll(fonts, false);
                    if !text.trim().is_empty()
                        && buffer.layout_runs().all(|run| run.glyphs.is_empty())
                    {
                        return Err("Compiled text produced no bundled-font glyphs".into());
                    }
                    let renderer =
                        TextRenderer::new(atlas, device, wgpu::MultisampleState::default(), None);
                    items.push(Item::Text(Box::new(TextItem {
                        index,
                        buffer,
                        renderer,
                    })));
                }
                Content::Image { resource } => {
                    items.push(Item::Image {
                        offset: (index * stride) as u32,
                        resource: *resource,
                    });
                }
            }
        }
        eprintln!(
            "Compiled original {example:?} fixture: nodes={}, canvas={}x{}, no external input; text line-height=1.25, no wrap, own-box clipping.",
            scene.scene().nodes.len(),
            scene.scene().width,
            scene.scene().height
        );
        Ok(Self {
            scene: Arc::new(scene),
            snapshot_key: None,
            items,
            pipeline,
            image_pipeline,
            images,
            uniform,
            bind_group,
            stride,
        })
    }
    pub fn clear_color(&self) -> wgpu::Color {
        let rgba = linear_color(self.scene.scene().background.0);
        wgpu::Color {
            r: rgba[0] as f64,
            g: rgba[1] as f64,
            b: rgba[2] as f64,
            a: rgba[3] as f64,
        }
    }

    pub fn sync_clock(&mut self, clock: &clock::Clock, fonts: &mut FontSystem) -> Result<()> {
        let Some(snapshot) = clock.snapshot_after(self.snapshot_key) else {
            return Ok(());
        };
        let before = self.scene.scene();
        let next = snapshot.scene.scene();
        if !same_topology(before, next) {
            return Err("Clock snapshot changed renderer topology".into());
        }
        for item in &mut self.items {
            if let Item::Text(item) = item
                && let Content::Text { text: old, .. } = &before.nodes[item.index].content
                && let Content::Text { text: new, .. } = &next.nodes[item.index].content
                && old != new
            {
                item.buffer.set_text(
                    new,
                    &Attrs::new().family(Family::Name("Noto Sans")),
                    Shaping::Advanced,
                    None,
                );
                item.buffer.shape_until_scroll(fonts, false);
            }
        }
        self.scene = snapshot.scene;
        self.snapshot_key = Some(snapshot.key);
        Ok(())
    }
    pub fn prepare(&mut self, state: &mut Prepare<'_>) -> Result<()> {
        let width = state.width as f32;
        let height = state.height as f32;
        let (scale, x, y) = fit(
            self.scene.scene().width,
            self.scene.scene().height,
            width,
            height,
        );
        let mut bytes = vec![0u8; self.stride * self.scene.scene().nodes.len().max(1)];
        for (index, node) in self.scene.scene().nodes.iter().enumerate() {
            let rgba = match node.content {
                Content::Rectangle { color } | Content::Text { color, .. } => linear_color(color.0),
                _ => [0.0; 4],
            };
            let values = [
                node.bounds.x * scale + x,
                node.bounds.y * scale + y,
                node.bounds.width * scale,
                node.bounds.height * scale,
                rgba[0],
                rgba[1],
                rgba[2],
                rgba[3],
                width,
                height,
                0.0,
                0.0,
            ];
            for (component, value) in values.iter().enumerate() {
                let offset = index * self.stride + component * 4;
                bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        state.queue.write_buffer(&self.uniform, 0, &bytes);
        for item in &mut self.items {
            if let Item::Text(text) = item {
                let node = &self.scene.scene().nodes[text.index];
                let Content::Text { color, .. } = node.content else {
                    unreachable!()
                };
                let left = node.bounds.x * scale + x;
                let top = node.bounds.y * scale + y;
                text.renderer.prepare(
                    state.device,
                    state.queue,
                    state.fonts,
                    state.atlas,
                    state.viewport,
                    [TextArea {
                        buffer: &text.buffer,
                        left,
                        top,
                        scale,
                        bounds: TextBounds {
                            left: left.floor().clamp(0.0, width) as i32,
                            top: top.floor().clamp(0.0, height) as i32,
                            right: (left + node.bounds.width * scale).ceil().clamp(0.0, width)
                                as i32,
                            bottom: (top + node.bounds.height * scale).ceil().clamp(0.0, height)
                                as i32,
                        },
                        default_color: Color::rgba(color.0[0], color.0[1], color.0[2], color.0[3]),
                        custom_glyphs: &[],
                    }],
                    state.swash,
                )?;
            }
        }
        Ok(())
    }
    pub fn render(
        &self,
        atlas: &TextAtlas,
        viewport: &Viewport,
        pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<()> {
        for item in &self.items {
            match item {
                Item::Rectangle(offset) => {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[*offset]);
                    pass.draw(0..6, 0..1);
                }
                Item::Text(text) => text.renderer.render(atlas, viewport, pass)?,
                Item::Image { offset, resource } => {
                    let image = self
                        .images
                        .get(resource)
                        .ok_or("Missing validated GPU image")?;
                    pass.set_pipeline(&self.image_pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[*offset]);
                    pass.set_bind_group(1, image, &[]);
                    pass.draw(0..6, 0..1);
                }
            }
        }
        Ok(())
    }
}

fn same_topology(before: &wallpaper_scene::Scene, after: &wallpaper_scene::Scene) -> bool {
    before.version == after.version
        && before.width == after.width
        && before.height == after.height
        && before.resources == after.resources
        && before.nodes.len() == after.nodes.len()
        && before.nodes.iter().zip(&after.nodes).all(|(left, right)| {
            left.id == right.id
                && left.bounds == right.bounds
                && match (&left.content, &right.content) {
                    (Content::Rectangle { .. }, Content::Rectangle { .. }) => true,
                    (Content::Text { font_size: a, .. }, Content::Text { font_size: b, .. }) => {
                        a == b
                    }
                    (Content::Image { resource: a }, Content::Image { resource: b }) => a == b,
                    _ => false,
                }
        })
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn clock_snapshot_accepts_text_changes_but_rejects_new_geometry_or_font() {
        let scene = clock::initial_scene().unwrap();
        let mut updated = scene.scene().clone();
        let Content::Text { text, .. } = &mut updated.nodes[1].content else {
            panic!()
        };
        *text = "23:59".into();
        assert!(same_topology(scene.scene(), &updated));
        let mut resized = updated.clone();
        resized.nodes[1].bounds.width -= 1.0;
        assert!(!same_topology(scene.scene(), &resized));
        let Content::Text { font_size, .. } = &mut updated.nodes[1].content else {
            panic!()
        };
        *font_size += 1.0;
        assert!(!same_topology(scene.scene(), &updated));
    }
}
pub struct Prepare<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub fonts: &'a mut FontSystem,
    pub atlas: &'a mut TextAtlas,
    pub viewport: &'a Viewport,
    pub swash: &'a mut SwashCache,
    pub width: u32,
    pub height: u32,
}
fn fit(canvas_width: u32, canvas_height: u32, width: f32, height: f32) -> (f32, f32, f32) {
    let scale = (width / canvas_width as f32).min(height / canvas_height as f32);
    (
        scale,
        (width - canvas_width as f32 * scale) * 0.5,
        (height - canvas_height as f32 * scale) * 0.5,
    )
}
fn linear_color(color: [u8; 4]) -> [f32; 4] {
    let channel = |byte: u8| {
        let value = f32::from(byte) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    [
        channel(color[0]),
        channel(color[1]),
        channel(color[2]),
        f32::from(color[3]) / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiled_shader_is_valid_without_a_device_or_window() {
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("compiled.wgsl")).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
        assert!(
            module
                .entry_points
                .iter()
                .any(|entry| entry.name == "fs_image")
        );
    }
    #[test]
    fn fixed_canvas_letterboxing_preserves_aspect_and_positions() {
        assert_eq!(fit(1920, 1080, 960.0, 600.0), (0.5, 0.0, 30.0));
        assert_eq!(fit(1920, 1080, 1920.0, 1080.0), (1.0, 0.0, 0.0));
    }
    #[test]
    fn straight_srgb_colors_convert_rgb_without_gamma_changing_alpha() {
        let color = linear_color([255, 0, 128, 128]);
        assert_eq!(color[0], 1.0);
        assert_eq!(color[1], 0.0);
        assert!((color[2] - 0.21586).abs() < 0.00001);
        assert_eq!(color[3], 128.0 / 255.0);
    }
}
