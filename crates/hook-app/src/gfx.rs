use hook_core::Image;
use hook_ui::gpu::UiPainter;
use hook_ui::{Css, Input, Key, Mods, State, Ui};
use std::sync::Arc;
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window};

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

pub struct View {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub painter: UiPainter,
    pub state: State,
    pub input: Input,
    pub mouse: (f32, f32),
    pub mods: ModifiersState,
}

impl Gpu {
    pub fn new(window: Arc<Window>) -> (Gpu, wgpu::Surface<'static>) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("no compatible GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).expect("could not open the GPU device");
        (Gpu { instance, adapter, device, queue }, surface)
    }

    fn step(&self) -> f64 {
        std::env::var("HOOK_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(120.0)
    }

    pub fn headless() -> Gpu {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("no GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).expect("could not open the GPU device");
        Gpu { instance, adapter, device, queue }
    }

    pub fn view(&self, window: Arc<Window>, surface: Option<wgpu::Surface<'static>>) -> View {
        let surface = surface.unwrap_or_else(|| self.instance.create_surface(window.clone()).expect("surface"));
        let caps = surface.get_capabilities(&self.adapter);
        let format = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
            .into_iter()
            .find(|f| caps.formats.contains(f))
            .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let mut config = surface.get_default_config(&self.adapter, size.width.max(1), size.height.max(1)).expect("config");
        config.format = format;
        if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            config.alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
        }
        surface.configure(&self.device, &config);
        View {
            painter: UiPainter::new(&self.device, format),
            window,
            surface,
            config,
            state: State::default(),
            input: Input::default(),
            mouse: (0.0, 0.0),
            mods: ModifiersState::default(),
        }
    }

    pub fn frames(&self, w: u32, h: u32, scale: f32, css: &Css, now: f64, n: usize, mut draw: impl FnMut(&mut Ui, usize)) -> Image {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut painter = UiPainter::new(&self.device, format);
        let mut state = State::default();
        let input = Input::default();
        for i in 0..n {
            painter.begin(w, h, scale);
            let mut ui = Ui::begin(&mut painter, &input, &mut state, css, now + i as f64 * self.step(), w as f32 / scale, h as f32 / scale);
            draw(&mut ui, i);
            ui.end();
        }
        render(&self.device, &self.queue, &mut painter, &view);
        read(&self.device, &self.queue, &texture)
    }
}

pub fn render(device: &wgpu::Device, queue: &wgpu::Queue, painter: &mut UiPainter, view: &wgpu::TextureView) {
    let mut enc = device.create_command_encoder(&Default::default());
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    painter.render(device, queue, &mut enc, view);
    queue.submit([enc.finish()]);
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Image {
    let (w, h) = (texture.width(), texture.height());
    let row = (w * 4).next_multiple_of(256);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        texture.size(),
    );
    queue.submit([enc.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let data = buffer.slice(..).get_mapped_range();
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        rgba.extend_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]);
    }
    drop(data);
    buffer.unmap();
    Image { w, h, rgba }
}

impl View {
    pub fn resize(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        self.config.width = w.max(1);
        self.config.height = h.max(1);
        self.surface.configure(device, &self.config);
    }

    pub fn scale(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    pub fn logical(&self) -> (f32, f32) {
        let s = self.scale();
        (self.config.width as f32 / s, self.config.height as f32 / s)
    }

    pub fn frame(&mut self, gpu: &Gpu, css: &Css, now: f64, draw: impl FnOnce(&mut Ui)) -> bool {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&gpu.device, &self.config);
                self.window.request_redraw();
                return false;
            }
        };
        let target = frame.texture.create_view(&Default::default());
        let scale = self.scale();
        let mut input = std::mem::take(&mut self.input);
        self.input.down = input.down;
        input.mouse = self.mouse;
        input.inside = true;
        input.mods = mods(self.mods);
        let busy = input.pressed || input.released || !input.keys.is_empty() || !input.text.is_empty() || input.wheel != (0.0, 0.0);
        let (lw, lh) = self.logical();
        self.painter.begin(self.config.width, self.config.height, scale);
        let mut ui = Ui::begin(&mut self.painter, &input, &mut self.state, css, now, lw, lh);
        draw(&mut ui);
        ui.end();
        render(&gpu.device, &gpu.queue, &mut self.painter, &target);
        self.window.pre_present_notify();
        frame.present();
        self.window.set_cursor(match self.state.cursor {
            hook_ui::Cursor::Pointer => CursorIcon::Pointer,
            hook_ui::Cursor::Text => CursorIcon::Text,
            hook_ui::Cursor::Grab => CursorIcon::Grab,
            hook_ui::Cursor::Grabbing => CursorIcon::Grabbing,
            hook_ui::Cursor::Crosshair => CursorIcon::Crosshair,
            _ => CursorIcon::Default,
        });
        let again = self.state.animating || busy;
        if again {
            self.window.request_redraw();
        }
        again
    }
}

pub fn ui_key(k: &WKey) -> Option<Key> {
    Some(match k {
        WKey::Named(NamedKey::Space) => Key::Char(' '),
        WKey::Named(NamedKey::ArrowUp) => Key::Up,
        WKey::Named(NamedKey::ArrowDown) => Key::Down,
        WKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WKey::Named(NamedKey::ArrowRight) => Key::Right,
        WKey::Named(NamedKey::Enter) => Key::Enter,
        WKey::Named(NamedKey::Escape) => Key::Escape,
        WKey::Named(NamedKey::Backspace) => Key::Backspace,
        WKey::Named(NamedKey::Delete) => Key::Delete,
        WKey::Named(NamedKey::Home) => Key::Home,
        WKey::Named(NamedKey::End) => Key::End,
        WKey::Named(NamedKey::Tab) => Key::Tab,
        WKey::Character(c) => Key::Char(c.chars().next()?.to_ascii_lowercase()),
        _ => return None,
    })
}

pub fn mods(m: ModifiersState) -> Mods {
    Mods {
        shift: m.shift_key(),
        ctrl: m.control_key(),
        alt: m.alt_key(),
        meta: m.super_key(),
    }
}
