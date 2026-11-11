use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let mut seconds = None;
    let mut desktop_mode = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!(
                    "Usage: wallpaper-preview [--desktop] [--seconds 1..3600]\nTrusted built-in Windows D3D12 scene. Escape closes the preview."
                );
                return Ok(());
            }
            "--desktop" => {
                desktop_mode = true;
            }
            "--seconds" => {
                let n: u64 = args.next().ok_or("Missing seconds")?.parse()?;
                if !(1..=3600).contains(&n) {
                    return Err("Seconds must be 1..3600".into());
                }
                seconds = Some(n);
            }
            _ => return Err(format!("Unsupported argument: {arg}").into()),
        }
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        state: None,
        desktop_mode,
        desktop: None,
        next_probe: Instant::now(),
        recoveries: 0,
        deadline: seconds.map(|s| Instant::now() + Duration::from_secs(s)),
        error: None,
        frames: 0,
    };
    event_loop.run_app(&mut app)?;
    eprintln!("Preview stopped; presented frames={}", app.frames);
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}

struct App {
    desktop_mode: bool,
    desktop: Option<wallpaper_platform::Desktop>,
    next_probe: Instant,
    recoveries: u32,
    state: Option<State>,
    deadline: Option<Instant>,
    error: Option<String>,
    frames: u64,
}
impl App {
    fn fail(&mut self, events: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.error = Some(error.to_string());
        self.state = None;
        events.exit();
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let result = (|| -> Result<State> {
            let desktop = if self.desktop_mode {
                Some(wallpaper_platform::Desktop::connect()?)
            } else {
                None
            };
            let attrs = Window::default_attributes()
                .with_title("Sahne — M1 trusted scene")
                .with_inner_size(LogicalSize::new(960.0, 600.0));
            let attrs = if let Some(ref d) = desktop {
                d.attributes(attrs)?
            } else {
                attrs
            };
            let window = Arc::new(events.create_window(attrs)?);
            self.desktop = desktop;
            pollster::block_on(State::new(window, events))
        })();
        match result {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(e) if self.desktop_mode && self.recoveries < 10 => {
                self.recoveries += 1;
                eprintln!("Desktop connection attempt {} failed: {e}", self.recoveries);
                self.next_probe = Instant::now() + Duration::from_secs(2);
            }
            Err(e) => self.fail(events, e),
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.state = None;
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        let now = Instant::now();
        if self.deadline.is_some_and(|d| now >= d) {
            if self.frames == 0 {
                self.error = Some("No frame was presented before the deadline".into());
            }
            events.exit();
            return;
        }
        if self.desktop_mode && now >= self.next_probe {
            if self.desktop.as_ref().is_some_and(|d| !d.alive()) {
                self.state = None;
                self.desktop = None;
            }
            if self.state.is_none() {
                self.resumed(events);
            }
            self.next_probe = Instant::now() + Duration::from_secs(2);
        }
        let wake = match (self.deadline, self.desktop_mode) {
            (Some(d), true) => Some(d.min(self.next_probe)),
            (None, true) => Some(self.next_probe),
            (d, false) => d,
        };
        events.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut().filter(|s| s.window.id() == id) else {
            return;
        };
        if matches!(event, WindowEvent::Destroyed) {
            self.state = None;
            self.desktop = None;
            return;
        }
        let result = match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                events.exit();
                Ok(())
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                state.resize();
                state.window.request_redraw();
                Ok(())
            }
            WindowEvent::RedrawRequested => state.draw().map(|presented| {
                if presented {
                    self.frames += 1;
                    eprintln!("Presented frame {}", self.frames);
                }
            }),
            WindowEvent::Occluded(false) => {
                state.window.request_redraw();
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(events, error);
        }
    }
}

struct State {
    surface_retries: u8,
    surface: wgpu::Surface<'static>,
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    image: wgpu::BindGroup,
    fonts: FontSystem,
    text: Buffer,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    window: Arc<Window>,
}
impl State {
    async fn new(window: Arc<Window>, events: &ActiveEventLoop) -> Result<Self> {
        let mut desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            events.owned_display_handle(),
        ));
        desc.backends = wgpu::Backends::DX12;
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        eprintln!(
            "Adapter: {:?}; DPI scale={}",
            adapter.get_info(),
            window.scale_factor()
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("Unsupported surface")?;
        surface.configure(&device, &config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Trusted fixture"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fixture"),
            layout: None,
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
                targets: &[Some(config.format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Built-in RGBA image"),
            size: wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let pixels: Vec<u8> = (0..64)
            .flat_map(|p| {
                if (p / 8 + p % 8) % 2 == 0 {
                    [220, 150, 70, 255]
                } else {
                    [30, 95, 115, 255]
                }
            })
            .collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32),
                rows_per_image: Some(8),
            },
            wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&Default::default());
        let image = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
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
        let mut fonts = FontSystem::new();
        let mut text = Buffer::new(&mut fonts, Metrics::new(32.0, 44.0));
        text.set_size(Some(820.0), Some(140.0));
        text.set_text(
            "Sahne / M1\nStatic GPU scene: shape + text + image",
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            None,
        );
        text.shape_until_scroll(&mut fonts, false);
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, config.format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        Ok(Self {
            surface_retries: 0,
            surface,
            instance,
            device,
            queue,
            config,
            pipeline,
            image,
            fonts,
            text,
            swash: SwashCache::new(),
            viewport,
            atlas,
            text_renderer,
            window,
        })
    }
    fn retry_surface(&mut self) -> Result<()> {
        self.surface_retries += 1;
        if self.surface_retries > 3 {
            return Err("Surface recovery exhausted".into());
        }
        self.window.request_redraw();
        Ok(())
    }
    fn resize(&mut self) {
        let size = self.window.inner_size();
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
    fn draw(&mut self) -> Result<bool> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(false);
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.retry_surface()?;
                self.resize();
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.retry_surface()?;
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.resize();
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Surface validation failed".into());
            }
        };
        self.surface_retries = 0;
        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.config.width,
                height: self.config.height,
            },
        );
        self.text_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.fonts,
            &mut self.atlas,
            &self.viewport,
            [TextArea {
                buffer: &self.text,
                left: 40.0,
                top: 32.0,
                scale: self.window.scale_factor() as f32,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: self.config.width as i32,
                    bottom: self.config.height as i32,
                },
                default_color: Color::rgb(235, 238, 230),
                custom_glyphs: &[],
            }],
            &mut self.swash,
        )?;
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.image, &[]);
            pass.draw(0..3, 0..1);
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)?;
        }
        self.window.pre_present_notify();
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        self.atlas.trim();
        Ok(true)
    }
}
