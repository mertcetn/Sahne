//! Multi-pass rendering of the original fixed v2 fixture; no external input.
use super::*;
use glyphon::Wrap;
use std::collections::BTreeMap;
use wallpaper_scene::Rect;
mod plan;
use plan::{Paint, Plan, Quad, Texture};
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const RECORD: usize = 80;
#[derive(Clone, Copy, Debug)]
pub enum Fixture {
    Original,
    Effects,
    Animated,
}
struct Surface {
    texture: Option<wgpu::Texture>,
    view: wgpu::TextureView,
    bindings: wgpu::BindGroup,
}
struct TextSurface {
    surface: Surface,
    buffer: Buffer,
    renderer: TextRenderer,
    viewport: Viewport,
}
pub struct Renderer {
    isolation: Vec<u32>,
    plan: Plan,
    targets: Vec<Surface>,
    images: BTreeMap<u32, Surface>,
    texts: Vec<TextSurface>,
    atlas: TextAtlas,
    swash: SwashCache,
    text_ready: bool,
    rect: wgpu::RenderPipeline,
    straight: wgpu::RenderPipeline,
    premultiplied: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    stride: usize,
}
impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        fonts: &mut FontSystem,
        fixture: Fixture,
        test_pixels: bool,
    ) -> Result<Self> {
        if !format.is_srgb() {
            return Err("Group preview requires an sRGB surface".into());
        }
        if test_pixels && !matches!(fixture, Fixture::Effects) {
            return Err("Pixel readback is restricted to the fixed effects fixture".into());
        }
        let package = match fixture {
            Fixture::Original => wallpaper_compiler::fixtures::preview_groups_package()?,
            Fixture::Effects => wallpaper_compiler::fixtures::effect_groups_package()?,
            Fixture::Animated => wallpaper_compiler::fixtures::animated_shapes_package()?,
        };
        let compiled = wallpaper_compiler::compile_groups(
            &package,
            wallpaper_compiler::Viewport {
                width: 1920,
                height: 1080,
                background: wallpaper_scene::Color([12, 17, 22, 255]),
            },
        )?;
        let scene = wallpaper_scene::hierarchy::decode(&compiled.scene().encode())?;
        wallpaper_media::validate_hierarchy_assets(&scene, compiled.images())?;
        let isolation = if matches!(fixture, Fixture::Animated) {
            vec![compiled.symbols()["fade"]]
        } else {
            vec![]
        };
        let plan = if isolation.is_empty() {
            Plan::new(&scene, device.limits().max_texture_dimension_2d)?
        } else {
            Plan::animated(&scene, device.limits().max_texture_dimension_2d, &isolation)?
        };
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        if alignment == 0 {
            return Err("Invalid group uniform alignment".into());
        }
        let stride = RECORD.div_ceil(alignment) * alignment;
        let size = stride
            .checked_mul(plan.quads.len() + 1)
            .ok_or("Group uniform size overflow")?;
        if size as u64 > device.limits().max_buffer_size || size > u32::MAX as usize {
            return Err("Group uniforms exceed device limits".into());
        }
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Group quad uniforms"),
            size: size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Group quad layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(RECORD as u64),
                },
                count: None,
            }],
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Group quad bindings"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: wgpu::BufferSize::new(RECORD as u64),
                }),
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Group texture layout"),
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
        let rect_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Group rectangle pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let image_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Group textured pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Group fixed shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("groups.wgsl").into()),
        });
        let rect = pipeline(
            device,
            &rect_layout,
            &shader,
            FORMAT,
            "fs_rect",
            wgpu::BlendState::ALPHA_BLENDING,
        );
        let straight = pipeline(
            device,
            &image_layout,
            &shader,
            FORMAT,
            "fs_straight",
            wgpu::BlendState::ALPHA_BLENDING,
        );
        let premultiplied = pipeline(
            device,
            &image_layout,
            &shader,
            FORMAT,
            "fs_premultiplied",
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
        );
        let present = pipeline(
            device,
            &image_layout,
            &shader,
            format,
            "fs_premultiplied",
            wgpu::BlendState::REPLACE,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Group nearest clamp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let targets = (0..plan.targets)
            .map(|i| {
                surface(
                    device,
                    &texture_layout,
                    &sampler,
                    plan.width,
                    plan.height,
                    test_pixels && i == 0,
                )
            })
            .collect();
        let mut images = BTreeMap::new();
        for (&id, image) in compiled.images() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Group validated image"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
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
                texture.size(),
            );
            images.insert(
                id,
                bind_surface(
                    device,
                    &texture_layout,
                    &sampler,
                    texture.create_view(&Default::default()),
                ),
            );
        }
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, FORMAT);
        let mut texts = vec![];
        for text in &plan.texts {
            let mut buffer =
                Buffer::new(fonts, Metrics::new(text.font_size, text.font_size * 1.25));
            buffer.set_size(Some(text.width), Some(text.height));
            buffer.set_wrap(Wrap::None);
            buffer.set_text(
                &text.text,
                &Attrs::new().family(Family::Name("Noto Sans")),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(fonts, false);
            if !text.text.trim().is_empty() && buffer.layout_runs().all(|run| run.glyphs.is_empty())
            {
                return Err("Group text produced no bundled glyphs".into());
            }
            let mut viewport = Viewport::new(device, &cache);
            viewport.update(
                queue,
                Resolution {
                    width: text.pixels[0],
                    height: text.pixels[1],
                },
            );
            texts.push(TextSurface {
                surface: surface(
                    device,
                    &texture_layout,
                    &sampler,
                    text.pixels[0],
                    text.pixels[1],
                    false,
                ),
                buffer,
                renderer: TextRenderer::new(
                    &mut atlas,
                    device,
                    wgpu::MultisampleState::default(),
                    None,
                ),
                viewport,
            });
        }
        let mut records = vec![0u8; size];
        for (index, quad) in plan.quads.iter().enumerate() {
            write_record(
                &mut records[index * stride..],
                quad,
                [plan.width, plan.height],
            );
        }
        queue.write_buffer(&uniform, 0, &records);
        eprintln!(
            "Compiled original Groups {fixture:?} fixture: layers={}, commands={}, offscreen targets={}, text surfaces={}, known texture bytes={}.",
            scene.scene().layers.len(),
            scene.commands().len(),
            plan.targets,
            plan.texts.len(),
            plan.texture_bytes
        );
        Ok(Self {
            isolation,
            plan,
            targets,
            images,
            texts,
            atlas,
            swash: SwashCache::new(),
            text_ready: false,
            rect,
            straight,
            premultiplied,
            present,
            uniform,
            bindings,
            stride,
        })
    }
    pub fn update_motion(
        &mut self,
        scene: &wallpaper_scene::hierarchy::ValidatedHierarchy,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        if self.isolation.is_empty() {
            return Err("Motion updates require the fixed animated fixture".into());
        }
        let next = Plan::animated(
            scene,
            device.limits().max_texture_dimension_2d,
            &self.isolation,
        )?;
        compatible_motion(&self.plan, &next)?;
        let mut records = vec![0; self.stride * self.plan.quads.len()];
        for (index, quad) in next.quads.iter().enumerate() {
            write_record(
                &mut records[index * self.stride..],
                quad,
                [next.width, next.height],
            );
        }
        queue.write_buffer(&self.uniform, 0, &records);
        self.plan = next;
        Ok(())
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        fonts: &mut FontSystem,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if !self.text_ready {
            for (index, text) in self.texts.iter_mut().enumerate() {
                let spec = &self.plan.texts[index];
                text.renderer.prepare(
                    device,
                    queue,
                    fonts,
                    &mut self.atlas,
                    &text.viewport,
                    [TextArea {
                        buffer: &text.buffer,
                        left: 0.0,
                        top: 0.0,
                        scale: spec.raster_scale,
                        bounds: TextBounds {
                            left: 0,
                            top: 0,
                            right: spec.pixels[0] as i32,
                            bottom: spec.pixels[1] as i32,
                        },
                        default_color: Color::rgba(
                            spec.color.0[0],
                            spec.color.0[1],
                            spec.color.0[2],
                            spec.color.0[3],
                        ),
                        custom_glyphs: &[],
                    }],
                    &mut self.swash,
                )?;
            }
        }
        let quad = Quad {
            bounds: plan::fit([self.plan.width, self.plan.height], [width, height]),
            clip: Rect {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
            },
            color: [1.0; 4],
            opacity: 1.0,
            uv: [1.0; 2],
        };
        let mut record = [0u8; RECORD];
        write_record(&mut record, &quad, [width, height]);
        queue.write_buffer(
            &self.uniform,
            (self.plan.quads.len() * self.stride) as u64,
            &record,
        );
        Ok(())
    }
    pub fn pixel_source(&self) -> Result<&wgpu::Texture> {
        self.targets[0]
            .texture
            .as_ref()
            .ok_or_else(|| "Pixel readback was not enabled".into())
    }

    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if !self.text_ready {
            for text in &self.texts {
                let mut pass = pass(
                    encoder,
                    &text.surface.view,
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                );
                text.renderer
                    .render(&self.atlas, &text.viewport, &mut pass)?;
            }
            self.text_ready = true;
        }
        for (index, target) in self.targets.iter().enumerate() {
            let clear = if index == 0 {
                self.clear_color()
            } else {
                wgpu::Color::TRANSPARENT
            };
            let _pass = pass(encoder, &target.view, wgpu::LoadOp::Clear(clear));
        }
        for op in &self.plan.ops {
            let quad = &self.plan.quads[op.quad];
            let Some(scissor) = plan::scissor(quad.clip, self.plan.width, self.plan.height) else {
                continue;
            };
            let mut pass = pass(encoder, &self.targets[op.target].view, wgpu::LoadOp::Load);
            pass.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3]);
            pass.set_bind_group(0, &self.bindings, &[(op.quad * self.stride) as u32]);
            match op.paint {
                Paint::Rectangle => pass.set_pipeline(&self.rect),
                Paint::Straight(texture) => {
                    pass.set_pipeline(&self.straight);
                    pass.set_bind_group(1, &self.texture(texture)?.bindings, &[]);
                }
                Paint::Premultiplied(texture) => {
                    pass.set_pipeline(&self.premultiplied);
                    pass.set_bind_group(1, &self.texture(texture)?.bindings, &[]);
                }
            }
            pass.draw(0..6, 0..1);
        }
        let mut pass = pass(encoder, output, wgpu::LoadOp::Clear(self.clear_color()));
        pass.set_scissor_rect(0, 0, width, height);
        pass.set_pipeline(&self.present);
        pass.set_bind_group(
            0,
            &self.bindings,
            &[(self.plan.quads.len() * self.stride) as u32],
        );
        pass.set_bind_group(1, &self.targets[0].bindings, &[]);
        pass.draw(0..6, 0..1);
        Ok(())
    }
    fn texture(&self, key: Texture) -> Result<&Surface> {
        match key {
            Texture::Image(id) => self.images.get(&id),
            Texture::Target(i) => self.targets.get(i),
            Texture::Text(i) => self.texts.get(i).map(|v| &v.surface),
        }
        .ok_or_else(|| "Missing validated group GPU resource".into())
    }
    fn clear_color(&self) -> wgpu::Color {
        let c = plan::linear_color(self.plan.background.0);
        wgpu::Color {
            r: (c[0] * c[3]) as f64,
            g: (c[1] * c[3]) as f64,
            b: (c[2] * c[3]) as f64,
            a: c[3] as f64,
        }
    }
}
fn compatible_motion(previous: &Plan, next: &Plan) -> Result<()> {
    if previous.width != next.width
        || previous.height != next.height
        || previous.background != next.background
        || previous.targets != next.targets
        || previous.texture_bytes != next.texture_bytes
        || previous.quads.len() != next.quads.len()
        || previous.ops != next.ops
        || previous.texts != next.texts
    {
        return Err(
            "Animated plan changed fixed GPU topology or text raster; refusing update".into(),
        );
    }
    Ok(())
}
fn write_record(bytes: &mut [u8], quad: &Quad, size: [u32; 2]) {
    let values = [
        quad.bounds.x,
        quad.bounds.y,
        quad.bounds.width,
        quad.bounds.height,
        quad.color[0],
        quad.color[1],
        quad.color[2],
        quad.color[3],
        quad.clip.x,
        quad.clip.y,
        quad.clip.x + quad.clip.width,
        quad.clip.y + quad.clip.height,
        size[0] as f32,
        size[1] as f32,
        0.0,
        0.0,
        quad.opacity,
        quad.uv[0],
        quad.uv[1],
        0.0,
    ];
    for (i, value) in values.iter().enumerate() {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    entry: &str,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
fn surface(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    copy_source: bool,
) -> Surface {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Bounded group render surface"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | if copy_source {
                wgpu::TextureUsages::COPY_SRC
            } else {
                wgpu::TextureUsages::empty()
            },
        view_formats: &[],
    });
    let mut result = bind_surface(
        device,
        layout,
        sampler,
        texture.create_view(&Default::default()),
    );
    result.texture = copy_source.then_some(texture);
    result
}
fn bind_surface(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: wgpu::TextureView,
) -> Surface {
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Group surface bindings"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    Surface {
        view,
        bindings,
        texture: None,
    }
}
fn pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Bounded group pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}

#[cfg(test)]
mod tests;
