//! Wayland platform implementation using winit with layer-shell support

use crate::config::Config;
use crate::core::{AttachmentTarget, ClaudeState, Vec2};
use crate::input::{kwin_window_picker, TmuxMonitor, WindowInfo};
use crate::renderer::Uniforms;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

#[cfg(target_os = "linux")]
use winit::platform::wayland::{
    ActiveEventLoopExtWayland, Anchor, KeyboardInteractivity, Layer, WindowAttributesWayland,
    Window as WaylandWindow,
};

/// Wayland platform using winit with layer-shell
pub struct WaylandPlatform {
    config: Config,
}

impl WaylandPlatform {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    pub fn run(self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        let mut app = WaylandApp::new(self.config);
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}

struct RenderState {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct WindowState {
    window: Arc<dyn Window>,
    render_state: Option<RenderState>,
    width: u32,
    height: u32,
}

struct WaylandApp {
    config: Config,
    windows: HashMap<WindowId, WindowState>,
    claude_state: Option<ClaudeState>,
    pointer_x: f64,
    pointer_y: f64,
    is_hovering: bool,
    last_frame: Instant,
    running: bool,
    modifiers: Modifiers,
    window_picker_rx: Option<mpsc::Receiver<WindowInfo>>,
    tmux_monitor: TmuxMonitor,
}

impl WaylandApp {
    fn new(config: Config) -> Self {
        Self {
            config,
            windows: HashMap::new(),
            claude_state: None,
            pointer_x: 0.0,
            pointer_y: 0.0,
            is_hovering: false,
            last_frame: Instant::now(),
            running: true,
            modifiers: Modifiers::default(),
            window_picker_rx: None,
            tmux_monitor: TmuxMonitor::new(),
        }
    }

    /// Start window picker in a background thread
    fn start_window_picker(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.window_picker_rx = Some(rx);

        // Run in background thread to not block the event loop
        thread::spawn(move || {
            // Get window at cursor position immediately
            if let Some(info) = kwin_window_picker::pick_window() {
                let _ = tx.send(info);
            }
        });
    }

    fn create_render_state(window: Arc<dyn Window>, width: u32, height: u32) -> Option<RenderState> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).ok()?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;

        tracing::info!(adapter = adapter.get_info().name, "Selected GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("hydra-observer"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .ok()?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            caps.alpha_modes[0]
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create shader and pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Claude Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../renderer/shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Some(RenderState {
            surface,
            surface_config,
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
        })
    }

    fn render(&mut self, window_id: WindowId) {
        let Some(window_state) = self.windows.get(&window_id) else {
            return;
        };
        let Some(ref render) = window_state.render_state else {
            return;
        };
        let Some(ref claude_state) = self.claude_state else {
            return;
        };

        let uniforms = Uniforms::from_state(
            claude_state,
            (window_state.width, window_state.height),
            self.config.appearance.scale,
            (self.pointer_x as f32, self.pointer_y as f32),
        );
        render
            .queue
            .write_buffer(&render.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        match render.surface.get_current_texture() {
            Ok(output) => {
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    render
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    pass.set_pipeline(&render.pipeline);
                    pass.set_bind_group(0, &render.bind_group, &[]);
                    pass.draw(0..6, 0..1);
                }

                render.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            Err(wgpu::SurfaceError::Lost) => {
                render.surface.configure(&render.device, &render.surface_config);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                tracing::error!("Out of memory");
                self.running = false;
            }
            Err(e) => {
                tracing::warn!("Surface error: {:?}", e);
            }
        }
    }
}

impl ApplicationHandler for WaylandApp {
    fn resumed(&mut self, _event_loop: &dyn ActiveEventLoop) {
        // Nothing to do on resume
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        tracing::info!("Creating layer-shell surface");

        // Get first monitor
        let monitor = event_loop.available_monitors().into_iter().next();
        let monitor_size = monitor
            .as_ref()
            .and_then(|m| m.current_video_mode())
            .map(|m| m.size())
            .unwrap_or(PhysicalSize::new(1920, 1080));

        // Create window attributes with explicit size request
        let mut window_attrs = WindowAttributes::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_title("hydra-observer")
            .with_surface_size(monitor_size);

        // Configure layer-shell on Wayland
        #[cfg(target_os = "linux")]
        if event_loop.is_wayland() {
            let wayland_attrs = WindowAttributesWayland::default()
                .with_layer_shell()
                .with_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT)
                .with_layer(Layer::Overlay)
                .with_keyboard_interactivity(KeyboardInteractivity::OnDemand)
                .with_margin(0, 0, 0, 0);  // No margins, full coverage

            // Bind to specific monitor if available
            if let Some(ref m) = monitor {
                let wayland_attrs = wayland_attrs.with_output(m.native_id());
                window_attrs = window_attrs.with_platform_attributes(Box::new(wayland_attrs));
            } else {
                window_attrs = window_attrs.with_platform_attributes(Box::new(wayland_attrs));
            }
        }

        // Create the window
        let window: Arc<dyn Window> = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::from(w),
            Err(e) => {
                tracing::error!("Failed to create window: {:?}", e);
                return;
            }
        };

        let width = monitor_size.width;
        let height = monitor_size.height;

        tracing::info!(width, height, "Layer surface created");

        // Create render state
        let render_state = Self::create_render_state(window.clone(), width, height);
        if render_state.is_none() {
            tracing::error!("Failed to create render state");
        }

        let window_id = window.id();
        self.windows.insert(
            window_id,
            WindowState {
                window,
                render_state,
                width,
                height,
            },
        );

        // Initialize claude state
        self.claude_state = Some(ClaudeState::new(&self.config));

        // Initialize cursor at center
        self.pointer_x = width as f64 / 2.0;
        self.pointer_y = height as f64 / 2.0;

        tracing::info!("Render pipeline created, starting main loop");
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if !self.running {
            event_loop.exit();
            return;
        }

        // Check for window picker results
        if let Some(ref rx) = self.window_picker_rx {
            match rx.try_recv() {
                Ok(info) => {
                    tracing::info!(
                        "Window selected: {} at ({}, {}) size {}x{}",
                        info.resource_name,
                        info.x,
                        info.y,
                        info.width,
                        info.height
                    );

                    if let Some(ref mut claude_state) = self.claude_state {
                        // Calculate target position (top-center of window, slightly inside)
                        let target_x = info.x as f32 + info.width as f32 / 2.0;
                        let target_y = info.y as f32 + 60.0; // Near top of window

                        let target = AttachmentTarget {
                            window_id: info.id(),
                            resource_name: info.resource_name.clone(),
                            position: Vec2::new(target_x, target_y),
                        };
                        claude_state.set_attachment_target(target);

                        // Start tmux monitoring if it's a terminal
                        if info.is_terminal() {
                            tracing::info!("Attached to terminal, starting tmux monitor");
                            self.tmux_monitor.detect_session();
                        }
                    }
                    self.window_picker_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Picker thread finished without sending result
                    tracing::info!("No window found at cursor position");
                    if let Some(ref mut claude_state) = self.claude_state {
                        claude_state.cancel_window_selection();
                    }
                    self.window_picker_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still waiting for picker result
                }
            }
        }

        // Update state
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        if let Some(ref mut claude_state) = self.claude_state {
            claude_state.set_cursor(self.pointer_x as f32, self.pointer_y as f32);
            claude_state.update(dt);

            // Update animated transitions
            claude_state.update_outline_transition(self.is_hovering, dt);
            let should_be_translucent = self.modifiers.state().shift_key() && claude_state.is_dragging();
            claude_state.update_translucent_transition(should_be_translucent, dt);
        }

        // Update input region to match mascot position
        if let Some(ref claude_state) = self.claude_state {
            for window_state in self.windows.values() {
                let screen_size = (window_state.width, window_state.height);
                let (x, y, w, h) = claude_state.get_input_region(screen_size);

                // Try to downcast to WaylandWindow to access set_region
                #[cfg(target_os = "linux")]
                if let Some(wayland_window) = (&*window_state.window).cast_ref::<WaylandWindow>() {
                    if let Ok(region) = wayland_window.create_region() {
                        region.add(x, y, w, h);
                        wayland_window.set_region(Some(&region));
                    }
                }
            }
        }

        // Request redraw for all windows
        for window_state in self.windows.values() {
            window_state.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested");
                self.running = false;
                event_loop.exit();
            }

            WindowEvent::SurfaceResized(size) => {
                tracing::info!(width = size.width, height = size.height, "Surface resized");
                if let Some(window_state) = self.windows.get_mut(&window_id) {
                    window_state.width = size.width;
                    window_state.height = size.height;
                    if let Some(ref render) = window_state.render_state {
                        let mut config = render.surface_config.clone();
                        config.width = size.width;
                        config.height = size.height;
                        render.surface.configure(&render.device, &config);
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                self.render(window_id);
            }

            WindowEvent::PointerMoved { position, .. } => {
                self.pointer_x = position.x;
                self.pointer_y = position.y;

                // Check if hovering over mascot
                if let Some(ref claude_state) = self.claude_state {
                    let screen_size = if let Some(ws) = self.windows.get(&window_id) {
                        (ws.width, ws.height)
                    } else {
                        (1920, 1080)
                    };
                    self.is_hovering = claude_state.contains_point(position.x as f32, position.y as f32, screen_size);
                }

                tracing::debug!("Pointer moved: ({}, {})", position.x, position.y);
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }

            WindowEvent::PointerButton {
                button,
                state,
                position,
                ..
            } => {
                tracing::debug!(
                    "Pointer button: {:?} {:?} at ({}, {}), shift={}",
                    button,
                    state,
                    position.x,
                    position.y,
                    self.modifiers.state().shift_key()
                );

                // Left click - toggle drag only if clicking on mascot
                if state == ElementState::Pressed {
                    if let Some(ref mut claude_state) = self.claude_state {
                        // Get window dimensions for hit test
                        let screen_size = if let Some(ws) = self.windows.get(&window_id) {
                            (ws.width, ws.height)
                        } else {
                            (1920, 1080)
                        };

                        // Check if click is on the mascot
                        if claude_state.contains_point(position.x as f32, position.y as f32, screen_size) {
                            if claude_state.is_dragging() {
                                // Was dragging, now releasing
                                if self.modifiers.state().shift_key() {
                                    // Shift held: trigger window picker
                                    tracing::info!("Shift+click detected - starting window picker");
                                    claude_state.await_window_selection();
                                    self.start_window_picker();
                                } else {
                                    // Normal release: just place
                                    claude_state.stop_drag();
                                }
                            } else {
                                // Not dragging: start drag
                                claude_state.start_drag();
                            }
                        }
                    }
                }
            }

            WindowEvent::PointerEntered { .. } => {
                tracing::info!("Pointer ENTERED our surface");
            }

            WindowEvent::PointerLeft { .. } => {
                self.is_hovering = false;
                // Reset pointer position to center when cursor leaves, so eyes look forward
                if let Some(ref claude_state) = self.claude_state {
                    self.pointer_x = claude_state.position.x as f64;
                    self.pointer_y = claude_state.position.y as f64;
                }
                tracing::info!("Pointer LEFT our surface");
            }

            _ => {}
        }
    }
}
