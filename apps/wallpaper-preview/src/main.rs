mod compiled;
mod groups;
mod lifecycle;
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
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[derive(Debug)]
struct DeviceLost {
    generation: u64,
    reason: String,
    explicitly_destroyed: bool,
}

fn main() -> Result<()> {
    let (mut seconds, mut desktop_mode, mut recreate) = (None, false, 0u32);
    let mut test_device_loss = false;
    let mut compiled_example = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!(
                    "Usage: wallpaper-preview [--desktop] [--example static|image|groups|groups-effects] [--seconds 1..3600] [--test-recreate 1..10 | --test-device-loss]\nTrusted Windows D3D12 scenes only. --example compiles original embedded HTML/CSS/PNG; no external file path. Escape closes window mode. Test recreation does not restart Explorer."
                );
                return Ok(());
            }
            "--desktop" => desktop_mode = true,
            "--example" => {
                compiled_example =
                    Some(match args.next().as_deref() {
                        Some("static") => compiled::Example::Static,
                        Some("image") => compiled::Example::Image,
                        Some("groups") => compiled::Example::Groups,
                        Some("groups-effects") => compiled::Example::GroupEffects,
                        _ => return Err(
                            "Choose an original static, image, groups or groups-effects example"
                                .into(),
                        ),
                    });
            }
            "--test-device-loss" => test_device_loss = true,
            "--seconds" => {
                let n = args.next().ok_or("Missing seconds")?.parse::<u64>()?;
                if !(1..=3600).contains(&n) {
                    return Err("Seconds must be 1..3600".into());
                }
                seconds = Some(n);
            }
            "--test-recreate" => {
                recreate = args.next().ok_or("Missing recreation count")?.parse()?;
                if !(1..=10).contains(&recreate) {
                    return Err("Recreation count must be 1..10".into());
                }
            }
            _ => return Err(format!("Unsupported argument: {arg}").into()),
        }
    }
    if test_device_loss && recreate > 0 {
        return Err("Choose one test mode per run".into());
    }
    let events = EventLoop::<DeviceLost>::with_user_event().build()?;
    events.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        state: None,
        desktop: None,
        desktop_mode,
        compiled_example,
        next_probe: Instant::now() + Duration::from_secs(2),
        next_retry: None,
        budget: Default::default(),
        generation: 0,
        proxy: events.create_proxy(),
        deadline: seconds.map(|s| Instant::now() + Duration::from_secs(s)),
        error: None,
        frames: 0,
        recreate_remaining: recreate,
        next_recreate: None,
        loss_test: lifecycle::DeviceLossTest::new(test_device_loss),
        next_device_test: None,
    };
    events.run_app(&mut app)?;
    eprintln!(
        "Preview stopped; presented frames={}; generations={}; pending recreations={}",
        app.frames, app.generation, app.recreate_remaining
    );
    eprintln!("Device loss test: {:?}", app.loss_test);
    if !app.loss_test.complete() {
        return Err("Device loss test did not observe callback and restored frame".into());
    }
    if app.recreate_remaining > 0 || app.next_recreate.is_some() {
        return Err("Recreation test interrupted before completion".into());
    }
    if let Some(e) = app.error {
        return Err(e.into());
    }
    Ok(())
}
struct App {
    state: Option<State>,
    desktop: Option<wallpaper_platform::Desktop>,
    desktop_mode: bool,
    compiled_example: Option<compiled::Example>,
    next_probe: Instant,
    next_retry: Option<Instant>,
    budget: lifecycle::RecoveryBudget,
    generation: u64,
    proxy: EventLoopProxy<DeviceLost>,
    deadline: Option<Instant>,
    error: Option<String>,
    frames: u64,
    recreate_remaining: u32,
    next_recreate: Option<Instant>,
    loss_test: lifecycle::DeviceLossTest,
    next_device_test: Option<Instant>,
}
impl App {
    fn fail(&mut self, events: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.error = Some(error.to_string());
        self.state = None;
        self.desktop = None;
        events.exit();
    }
    fn recover(&mut self, events: &ActiveEventLoop, reason: &str) {
        match self.budget.consume() {
            Ok(attempt) => {
                eprintln!("Recovery {attempt}/10: {reason}");
                self.state = None;
                self.desktop = None;
                self.next_retry = Some(Instant::now() + Duration::from_millis(500));
                self.next_recreate = None;
            }
            Err(e) => self.fail(events, e),
        }
    }
    fn create(&mut self, events: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        self.next_retry = None;
        self.generation += 1;
        let result = (|| -> Result<State> {
            let desktop = if self.desktop_mode {
                Some(wallpaper_platform::Desktop::connect()?)
            } else {
                None
            };
            let attrs = Window::default_attributes()
                .with_title(match self.compiled_example {
                    Some(compiled::Example::Static) => "Sahne — M3 compiled static example",
                    Some(compiled::Example::Image) => "Sahne — M3 compiled image example",
                    Some(compiled::Example::Groups) => "Sahne — M3 compiled groups example",
                    Some(compiled::Example::GroupEffects) => "Sahne — M3 group effects example",
                    None => "Sahne — M1 trusted scene",
                })
                .with_inner_size(LogicalSize::new(960.0, 600.0));
            let attrs = if let Some(ref d) = desktop {
                d.attributes(attrs)?
            } else {
                attrs
            };
            let window = Arc::new(events.create_window(attrs)?);
            self.desktop = desktop;
            pollster::block_on(State::new(
                window,
                events,
                self.proxy.clone(),
                self.generation,
                !self.loss_test.complete(),
                self.compiled_example,
            ))
        })();
        match result {
            Ok(state) => {
                eprintln!(
                    "Generation {} created: size={:?}, scale={}",
                    self.generation,
                    state.window.inner_size(),
                    state.window.scale_factor()
                );
                for monitor in events.available_monitors() {
                    eprintln!(
                        "Monitor: {:?} {:?} position={:?} scale={} refresh={:?}",
                        monitor.name(),
                        monitor.size(),
                        monitor.position(),
                        monitor.scale_factor(),
                        monitor.refresh_rate_millihertz()
                    );
                }
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(e) => self.recover(events, &format!("Initialization failed: {e}")),
        }
    }
}
impl ApplicationHandler<DeviceLost> for App {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.next_retry.is_none() {
            self.create(events);
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.state = None;
        self.desktop = None;
        self.next_recreate = None;
    }
    fn user_event(&mut self, events: &ActiveEventLoop, loss: DeviceLost) {
        if loss.generation == self.generation && self.state.is_some() {
            self.loss_test
                .callback(loss.generation, loss.explicitly_destroyed);
            self.recover(events, &loss.reason);
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        let now = Instant::now();
        if self.deadline.is_some_and(|d| now >= d) {
            if self.frames == 0
                || self.state.is_none()
                || self.next_retry.is_some()
                || self.recreate_remaining > 0
                || self.next_recreate.is_some()
            {
                self.error =
                    Some("Deadline reached before a healthy final scene/test completion".into());
            }
            events.exit();
            return;
        }
        if self.next_retry.is_some_and(|d| now >= d) {
            self.create(events);
        }
        if self.next_recreate.is_some_and(|d| now >= d) {
            self.recreate_remaining -= 1;
            self.recover(
                events,
                "Injected window/GPU teardown (not an Explorer restart)",
            );
        }
        if self.next_device_test.is_some_and(|d| now >= d) {
            self.next_device_test = None;
            if let Some(state) = self.state.as_ref() {
                self.loss_test.request(self.generation);
                eprintln!("Injecting device.destroy generation={}", self.generation);
                state.device.destroy();
                let _ = state.device.poll(wgpu::PollType::Poll);
            }
        }
        if self.desktop_mode && now >= self.next_probe {
            let size = self.desktop.as_ref().map(|d| d.size());
            match size {
                Some(Ok(size)) => {
                    if let Some(state) = self.state.as_ref()
                        && state.window.inner_size() != size
                    {
                        let _ = state.window.request_inner_size(size);
                        eprintln!("Desktop size changed: {size:?}");
                    }
                }
                Some(Err(e)) => self.recover(events, &format!("Desktop invalid: {e}")),
                None => {}
            }
            self.next_probe = Instant::now() + Duration::from_secs(2);
        }
        let wake = [
            self.deadline,
            self.next_retry,
            self.next_recreate,
            self.next_device_test,
            self.desktop_mode.then_some(self.next_probe),
        ]
        .into_iter()
        .flatten()
        .min();
        events.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !self.state.as_ref().is_some_and(|s| s.window.id() == id) {
            return;
        }
        if matches!(event, WindowEvent::Destroyed) {
            self.recover(events, "Native window destroyed");
            return;
        }
        if self
            .desktop
            .as_ref()
            .is_some_and(|desktop| !desktop.alive())
        {
            self.recover(events, "Desktop owner invalid before window event");
            return;
        }
        let state = self.state.as_mut().expect("checked window");
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
                eprintln!(
                    "Surface event: size={:?} scale={}",
                    state.window.inner_size(),
                    state.window.scale_factor()
                );
                state.resize();
                state.window.request_redraw();
                Ok(())
            }
            WindowEvent::RedrawRequested => state.draw().map(|presented| {
                if presented {
                    self.loss_test.frame(self.generation);
                    if self.loss_test.needs_request() && self.next_device_test.is_none() {
                        self.next_device_test = Some(Instant::now() + Duration::from_secs(1));
                    }
                    self.frames += 1;
                    eprintln!(
                        "Presented frame {} generation={}",
                        self.frames, self.generation
                    );
                    if self.recreate_remaining > 0 && self.next_recreate.is_none() {
                        self.next_recreate = Some(Instant::now() + Duration::from_secs(1));
                    }
                }
            }),
            WindowEvent::Occluded(false) => {
                state.window.request_redraw();
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.recover(events, &format!("Renderer error: {e}"));
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
    compiled: Option<compiled::Renderer>,
    groups: Option<groups::Renderer>,
    window: Arc<Window>,
}
impl State {
    async fn new(
        window: Arc<Window>,
        events: &ActiveEventLoop,
        proxy: EventLoopProxy<DeviceLost>,
        generation: u64,
        report_destroyed: bool,
        compiled_example: Option<compiled::Example>,
    ) -> Result<Self> {
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
        device.set_device_lost_callback(move |reason, message| {
            if reason != wgpu::DeviceLostReason::Destroyed || report_destroyed {
                let _ = proxy.send_event(DeviceLost {
                    generation,
                    reason: format!("Device lost: {reason:?}: {message}"),
                    explicitly_destroyed: reason == wgpu::DeviceLostReason::Destroyed,
                });
            }
        });
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("Unsupported surface")?;
        if compiled_example.is_some() {
            config.format = surface
                .get_capabilities(&adapter)
                .formats
                .into_iter()
                .find(wgpu::TextureFormat::is_srgb)
                .ok_or("Compiled example requires an sRGB surface")?;
        }
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
        let mut db = glyphon::fontdb::Database::new();
        db.load_font_data(include_bytes!("../../../assets/fonts/NotoSans.ttf").to_vec());
        db.set_sans_serif_family("Noto Sans");
        let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
        let mut text = Buffer::new(&mut fonts, Metrics::new(32.0, 44.0));
        text.set_size(Some(820.0), Some(140.0));
        text.set_text(
            "Sahne / M1\nStatic GPU scene: shape + text + image",
            &Attrs::new().family(Family::Name("Noto Sans")),
            Shaping::Advanced,
            None,
        );
        text.shape_until_scroll(&mut fonts, false);
        if text.layout_runs().all(|run| run.glyphs.is_empty()) {
            return Err("Bundled font produced no text glyphs".into());
        }
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, config.format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let groups = if let Some(fixture) =
            compiled_example.and_then(compiled::Example::group_fixture)
        {
            let memory_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
            let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let result = groups::Renderer::new(&device, &queue, config.format, &mut fonts, fixture);
            let validation = validation_scope.pop();
            let memory = memory_scope.pop();
            let validation = validation.await;
            let memory = memory.await;
            if let Some(error) = validation.or(memory) {
                return Err(format!("Group GPU initialization failed: {error}").into());
            }
            Some(result?)
        } else {
            None
        };
        let compiled =
            if let Some(example) = compiled_example.filter(|v| v.group_fixture().is_none()) {
                Some(compiled::Renderer::new(
                    &device,
                    &queue,
                    config.format,
                    &mut fonts,
                    &mut atlas,
                    example,
                )?)
            } else {
                None
            };
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
            compiled,
            groups,
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
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.retry_surface()?;
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
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
        if let Some(groups) = self.groups.as_mut() {
            groups.prepare(
                &self.device,
                &self.queue,
                &mut self.fonts,
                self.config.width,
                self.config.height,
            )?;
        } else if let Some(compiled) = self.compiled.as_mut() {
            compiled.prepare(&mut compiled::Prepare {
                device: &self.device,
                queue: &self.queue,
                fonts: &mut self.fonts,
                atlas: &mut self.atlas,
                viewport: &self.viewport,
                swash: &mut self.swash,
                width: self.config.width,
                height: self.config.height,
            })?;
        } else {
            self.text_renderer.prepare(
                &self.device,
                &self.queue,
                &mut self.fonts,
                &mut self.atlas,
                &self.viewport,
                [TextArea {
                    buffer: &self.text,
                    left: 40.0 * self.window.scale_factor() as f32,
                    top: 32.0 * self.window.scale_factor() as f32,
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
        }
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        if let Some(groups) = self.groups.as_mut() {
            groups.encode(&mut encoder, &view, self.config.width, self.config.height)?;
        } else {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(
                            self.compiled
                                .as_ref()
                                .map_or(wgpu::Color::BLACK, compiled::Renderer::clear_color),
                        ),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if let Some(compiled) = self.compiled.as_ref() {
                compiled.render(&self.atlas, &self.viewport, &mut pass)?;
            } else {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.image, &[]);
                pass.draw(0..3, 0..1);
                self.text_renderer
                    .render(&self.atlas, &self.viewport, &mut pass)?;
            }
        }
        self.window.pre_present_notify();
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        self.atlas.trim();
        Ok(true)
    }
}
