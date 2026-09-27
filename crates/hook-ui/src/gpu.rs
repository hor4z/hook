use crate::core::{Face, Painter, R};
use crate::text::fallback::{coverage, embedded, measure_char};
use crate::text::metrics::TextMetrics;
use crate::text::shaping_face;
use std::collections::HashMap;

fn icon_svg(name: &str) -> Option<&'static str> {
    if name == "arrow-right-solid" {
        return Some(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M3 9.5h10.5V5l8.5 7-8.5 7v-4.5H3z"/></svg>"#);
    }
    crate::text::icon_source(name)
}

pub fn render_svg(source: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(source, &opt).ok()?;
    let mut pix = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    let size = tree.size();
    let t = resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, t, &mut pix.as_mut());
    Some(pix.take())
}

pub fn icon_mask(name: &str, px: u32, fill: bool, stroke: Option<f32>) -> Option<Vec<u8>> {
    let mut src = icon_svg(name)?.replace("currentColor", "#fff");
    if fill {
        src = src.replace("fill=\"none\"", "fill=\"#fff\"");
    }
    if let Some(s) = stroke {
        src = src.replace("stroke-width=\"2\"", &format!("stroke-width=\"{s}\""));
    }
    let rgba = render_svg(&src, px, px)?;
    Some(rgba.chunks_exact(4).map(|p| p[3]).collect())
}

#[derive(Clone, Copy)]
pub struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub ox: i32,
    pub oy: i32,
}

pub struct Shelf {
    pub size: u32,
    pub data: Vec<u8>,
    pub channels: usize,
    x: u32,
    y: u32,
    row: u32,
    pub dirty: Option<(u32, u32, u32, u32)>,
    pub version: u64,
}

impl Shelf {
    pub fn new(size: u32, channels: usize) -> Shelf {
        Shelf {
            size,
            data: vec![0; (size * size) as usize * channels],
            channels,
            x: 2,
            y: 2,
            row: 0,
            dirty: None,
            version: 1,
        }
    }

    pub fn reset(&mut self) {
        self.data.iter_mut().for_each(|v| *v = 0);
        self.x = 2;
        self.y = 2;
        self.row = 0;
        self.dirty = None;
        self.version += 1;
    }

    pub fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.x + w + 2 > self.size {
            self.x = 2;
            self.y += self.row + 2;
            self.row = 0;
        }
        if self.y + h + 2 > self.size {
            return None;
        }
        let at = (self.x, self.y);
        self.x += w + 2;
        self.row = self.row.max(h);
        let d = self.dirty.unwrap_or((at.0, at.1, at.0 + w, at.1 + h));
        self.dirty = Some((d.0.min(at.0), d.1.min(at.1), d.2.max(at.0 + w), d.3.max(at.1 + h)));
        Some(at)
    }

    pub fn put(&mut self, at: (u32, u32), w: u32, h: u32, pixels: &[u8]) {
        let c = self.channels;
        for j in 0..h as usize {
            let dst = ((at.1 as usize + j) * self.size as usize + at.0 as usize) * c;
            self.data[dst..dst + w as usize * c].copy_from_slice(&pixels[j * w as usize * c..(j + 1) * w as usize * c]);
        }
    }

    pub fn take_dirty(&mut self) -> Option<(u32, u32, u32, u32)> {
        self.dirty.take()
    }
}

pub struct GlyphCache {
    pub shelf: Shelf,
    slots: HashMap<String, Option<Slot>>,
}

pub const SUB: f32 = 3.0;

impl GlyphCache {
    pub fn new(size: u32, channels: usize) -> GlyphCache {
        GlyphCache {
            shelf: Shelf::new(size, channels),
            slots: HashMap::new(),
        }
    }

    fn store(&mut self, key: String, w: u32, h: u32, ox: i32, oy: i32, alpha: &[u8]) -> Option<&Slot> {
        let at = match self.shelf.alloc(w, h) {
            Some(a) => a,
            None => {
                self.shelf.reset();
                self.slots.clear();
                self.shelf.alloc(w, h)?
            }
        };
        let px: Vec<u8> = if self.shelf.channels == 4 {
            alpha.iter().flat_map(|a| [255, 255, 255, *a]).collect()
        } else {
            alpha.to_vec()
        };
        self.shelf.put(at, w, h, &px);
        self.slots.insert(key.clone(), Some(Slot { x: at.0, y: at.1, w, h, ox, oy }));
        self.slots.get(&key).and_then(|s| s.as_ref())
    }

    pub fn glyph(&mut self, face: Face, ch: char, px: f32, sub: u8) -> Option<&Slot> {
        let key = format!("{face:?}|{px}|{sub}|{ch}");
        if self.slots.contains_key(&key) {
            return self.slots.get(&key).and_then(|s| s.as_ref());
        }
        if embedded(face).glyph(ch) != 0 {
            let r = crate::text::bitmap::Raster::default();
            return match r.glyph(shaping_face(face), ch, px, sub) {
                Some(g) => {
                    let (w, h, ox, oy) = (g.w, g.h, g.ox, g.oy);
                    self.store(key, w, h, ox, oy, &g.alpha)
                }
                None => {
                    self.slots.insert(key, None);
                    None
                }
            };
        }
        let m = measure_char(face, ch, px);
        let s = sub as f32 / SUB;
        let left = (-m.left + s).floor() as i32 - 1;
        let right = (m.right + s).ceil() as i32 + 1;
        let up = m.ascent.ceil() as i32 + 1;
        let down = m.descent.ceil() as i32 + 1;
        let w = (right - left).max(1) as u32;
        let h = (up + down).max(1) as u32;
        if m.right - (-m.left) <= 0.0 {
            self.slots.insert(key, None);
            return None;
        }
        let cov = coverage(face, ch, px, -left as f32 + s, up as f32, w as usize, h as usize);
        let alpha: Vec<u8> = cov.iter().map(|c| (c * 255.0).round() as u8).collect();
        self.store(key, w, h, left, -up, &alpha)
    }

    pub fn icon(&mut self, name: &str, px: u32, fill: bool, stroke: Option<f32>) -> Option<&Slot> {
        let key = format!("icon|{name}|{px}|{fill}|{stroke:?}");
        if self.slots.contains_key(&key) {
            return self.slots.get(&key).and_then(|s| s.as_ref());
        }
        if let Some(g) = crate::text::bitmap::Raster::default().icon(name, px, fill, stroke.unwrap_or(2.0)) {
            let (w, h, ox, oy) = (g.w, g.h, g.ox, g.oy);
            return self.store(key, w, h, ox, oy, &g.alpha);
        }
        let w = px + 2;
        let inner = icon_mask(name, px, fill, stroke).unwrap_or_default();
        let mut alpha = vec![0u8; (w * w) as usize];
        for y in 0..px {
            for x in 0..px {
                alpha[((y + 1) * w + x + 1) as usize] = inner.get((y * px + x) as usize).copied().unwrap_or(0);
            }
        }
        self.store(key, w, w, -1, -1, &alpha)
    }
}

impl GlyphCache {
    pub fn shaped(&mut self, face: crate::text::Face, id: u16, px: f32, level: usize) -> Option<&Slot> {
        let key = format!("s|{face:?}|{id}|{px}|{level}");
        if self.slots.contains_key(&key) {
            return self.slots.get(&key).and_then(|s| s.as_ref());
        }
        let r = crate::text::bitmap::Raster { level, ..Default::default() };
        match r.glyph_id(face, id, px, 0) {
            Some(g) if g.w > 0 && g.h > 0 => {
                let (w, h, ox, oy) = (g.w, g.h, g.ox, g.oy);
                self.store(key, w, h, ox, oy, &g.alpha)
            }
            _ => {
                self.slots.insert(key, None);
                None
            }
        }
    }
}

impl GlyphCache {
    pub fn canvas(&mut self, face: Face, ch: char, px: f32, sub: u8, level: usize) -> Option<&Slot> {
        let key = format!("c|{face:?}|{px}|{sub}|{level}|{ch}");
        if self.slots.contains_key(&key) {
            return self.slots.get(&key).and_then(|s| s.as_ref());
        }
        if embedded(face).glyph(ch) == 0 {
            return self.glyph(face, ch, px, sub);
        }
        let r = crate::text::bitmap::Raster {
            subpixel: true,
            level,
            ..Default::default()
        };
        match r.glyph(shaping_face(face), ch, px, sub) {
            Some(g) if g.w > 0 && g.h > 0 => {
                let (w, h, ox, oy) = (g.w, g.h, g.ox, g.oy);
                self.store(key, w, h, ox, oy, &g.alpha)
            }
            _ => {
                self.slots.insert(key, None);
                None
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Inst {
    rect: [f32; 4],
    radii: [f32; 4],
    color: [f32; 4],
    p: [f32; 4],
    uv: [f32; 4],
    clip: [f32; 4],
}

pub struct UiPainter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    view_buf: wgpu::Buffer,
    inst_buf: Option<(wgpu::Buffer, u64)>,
    glyph_tex: Option<(wgpu::Texture, wgpu::TextureView, u64)>,
    image_tex: Option<(wgpu::Texture, wgpu::TextureView, u64)>,
    glyphs: GlyphCache,
    images: GlyphCache,
    svgs: HashMap<String, Option<(u32, u32, u32, u32)>>,
    inst: Vec<Inst>,
    clips: Vec<[f32; 4]>,
    scale: f32,
    width: u32,
    height: u32,
    corner: f32,
    metrics: TextMetrics,
}

impl UiPainter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> UiPainter {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1),
                tex_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attrs: Vec<wgpu::VertexAttribute> = (0..6)
            .map(|i| wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: i * 16,
                shader_location: i as u32,
            })
            .collect();
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 96,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState { color: blend, alpha: blend }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let view_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-view"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        UiPainter {
            pipeline,
            layout,
            sampler,
            view_buf,
            inst_buf: None,
            glyph_tex: None,
            image_tex: None,
            glyphs: GlyphCache::new(2048, 1),
            images: GlyphCache::new(2048, 4),
            svgs: HashMap::new(),
            inst: Vec::new(),
            clips: Vec::new(),
            scale: 1.0,
            width: 1,
            height: 1,
            corner: 0.0,
            metrics: TextMetrics::new(),
        }
    }

    pub fn begin(&mut self, width: u32, height: u32, scale: f32) {
        self.inst.clear();
        self.clips.clear();
        self.scale = scale;
        self.width = width;
        self.height = height;
    }

    pub fn set_corner(&mut self, radius: f32) {
        self.corner = radius * self.scale;
    }

    fn clip(&self) -> [f32; 4] {
        self.clips.last().copied().unwrap_or([-1e6, -1e6, 1e6, 1e6])
    }

    fn push(&mut self, rect: [f32; 4], radii: [f32; 4], color: [f32; 4], p: [f32; 4], uv: [f32; 4]) {
        let clip = self.clip();
        self.inst.push(Inst { rect, radii, color, p, uv, clip });
    }

    fn texture(device: &wgpu::Device, queue: &wgpu::Queue, slot: &mut Option<(wgpu::Texture, wgpu::TextureView, u64)>, shelf: &mut Shelf, format: wgpu::TextureFormat) {
        let bpp = shelf.channels as u32;
        if slot.as_ref().is_none_or(|s| s.2 != shelf.version) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("ui-atlas"),
                size: wgpu::Extent3d {
                    width: shelf.size,
                    height: shelf.size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            *slot = Some((tex, view, shelf.version));
            shelf.dirty = Some((0, 0, shelf.size, shelf.size));
        }
        if let (Some((tex, _, _)), Some((x0, y0, x1, y1))) = (slot.as_ref(), shelf.take_dirty()) {
            let row = (shelf.size * bpp) as usize;
            let mut sub = Vec::with_capacity(((x1 - x0) * (y1 - y0) * bpp) as usize);
            for y in y0..y1 {
                let s = y as usize * row + (x0 * bpp) as usize;
                sub.extend_from_slice(&shelf.data[s..s + ((x1 - x0) * bpp) as usize]);
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: x0, y: y0, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &sub,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some((x1 - x0) * bpp),
                    rows_per_image: Some(y1 - y0),
                },
                wgpu::Extent3d {
                    width: x1 - x0,
                    height: y1 - y0,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        Self::texture(device, queue, &mut self.glyph_tex, &mut self.glyphs.shelf, wgpu::TextureFormat::R8Unorm);
        Self::texture(device, queue, &mut self.image_tex, &mut self.images.shelf, wgpu::TextureFormat::Rgba8Unorm);
        if self.inst.is_empty() {
            return;
        }
        queue.write_buffer(&self.view_buf, 0, bytemuck::cast_slice(&[self.width as f32, self.height as f32, self.corner, 0.0]));
        let bytes: &[u8] = bytemuck::cast_slice(&self.inst);
        if self.inst_buf.as_ref().is_none_or(|b| b.1 < bytes.len() as u64) {
            let size = (bytes.len() as u64).next_power_of_two().max(4096);
            self.inst_buf = Some((
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ui-inst"),
                    size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                size,
            ));
        }
        let buf = &self.inst_buf.as_ref().unwrap().0;
        queue.write_buffer(buf, 0, bytes);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.view_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.glyph_tex.as_ref().unwrap().1),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.image_tex.as_ref().unwrap().1),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_vertex_buffer(0, buf.slice(..bytes.len() as u64));
        pass.draw(0..6, 0..self.inst.len() as u32);
    }
}

impl Painter for UiPainter {
    fn rect(&mut self, r: R, radii: [f32; 4], fill: [f32; 4]) {
        let s = self.scale;
        self.push([r.x * s, r.y * s, r.w * s, r.h * s], radii.map(|v| v * s), fill, [0.0; 4], [0.0; 4]);
    }

    fn border(&mut self, r: R, radii: [f32; 4], color: [f32; 4], width: f32) {
        let s = self.scale;
        self.push([r.x * s, r.y * s, r.w * s, r.h * s], radii.map(|v| v * s), color, [1.0, width * s, 0.0, 0.0], [0.0; 4]);
    }

    fn shadow(&mut self, r: R, radius: f32, blur: f32, spread: f32, dy: f32, color: [f32; 4]) {
        let s = self.scale;
        self.push([r.x * s, r.y * s, r.w * s, r.h * s], [radius * s; 4], color, [2.0, blur * s, spread * s, dy * s], [0.0; 4]);
    }

    fn segment(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: [f32; 4]) {
        let s = self.scale;
        self.push([a.0 * s, a.1 * s, b.0 * s, b.1 * s], [0.0; 4], color, [5.0, width * s, 0.0, 0.0], [0.0; 4]);
    }

    fn segment_butt(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: [f32; 4]) {
        let s = self.scale;
        self.push([a.0 * s, a.1 * s, b.0 * s, b.1 * s], [0.0; 4], color, [5.0, width * s, 1.0, 0.0], [0.0; 4]);
    }

    fn arc(&mut self, c: (f32, f32), r: f32, start: f32, sweep: f32, width: f32, color: [f32; 4]) {
        let s = self.scale;
        self.push([c.0 * s, c.1 * s, r * s, width * s], [0.0; 4], color, [6.0, start, sweep, 0.0], [0.0; 4]);
    }

    fn text(&mut self, x: f32, baseline: f32, text: &str, face: Face, size: f32, color: [f32; 4], tracking: f32) -> f32 {
        let s = self.scale;
        let px = size * s;
        let base = (baseline * s).round();
        let n = self.glyphs.shelf.size as f32;
        let level = crate::text::gamma::level([color[0], color[1], color[2]]);
        let shaped = crate::text::shape::shape(shaping_face(face), text, size, if size > 0.0 { tracking / size } else { 0.0 });
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        for g in &shaped.glyphs {
            let gx = ((x + g.x) * s).round();
            let gy = base + (g.y * s).round();
            let slot = if g.glyph == 0 {
                let ch = chars.iter().find(|(i, _)| *i == g.cluster).map(|(_, c)| *c).unwrap_or(' ');
                if ch == ' ' {
                    None
                } else {
                    self.glyphs.glyph(face, ch, (px * 4.0).round() / 4.0, 0).map(|s| Slot { ..*s })
                }
            } else {
                self.glyphs.shaped(g.face, g.glyph, px, level).map(|s| Slot { ..*s })
            };
            if let Some(slot) = slot {
                let uv = [slot.x as f32 / n, slot.y as f32 / n, (slot.x + slot.w) as f32 / n, (slot.y + slot.h) as f32 / n];
                self.push([gx + slot.ox as f32, gy + slot.oy as f32, slot.w as f32, slot.h as f32], [0.0; 4], color, [3.0, 0.0, 0.0, 0.0], uv);
            }
        }
        x + shaped.advance
    }

    fn measure(&mut self, text: &str, face: Face, size: f32, tracking: f32) -> f32 {
        self.metrics.measure(text, face, size, tracking)
    }

    fn metrics(&mut self, face: Face) -> (f32, f32) {
        self.metrics.metrics(face)
    }

    fn text_canvas(&mut self, x: f32, baseline: f32, text: &str, face: Face, size: f32, color: [f32; 4]) -> f32 {
        let s = self.scale;
        let px = (size * s * 4.0).round() / 4.0;
        let base = (baseline * s).round();
        let mut u = x * s;
        let n = self.glyphs.shelf.size as f32;
        let level = crate::text::gamma::level([color[0], color[1], color[2]]);
        for ch in text.chars() {
            let adv = self.metrics.canvas_advance(face, ch, px);
            if ch != ' ' && ch != '\t' {
                let mut whole = u.floor();
                let mut sub = ((u - whole) * SUB).round();
                if sub == SUB {
                    whole += 1.0;
                    sub = 0.0;
                }
                if let Some(slot) = self.glyphs.canvas(face, ch, px, sub as u8, level).map(|s| Slot { ..*s }) {
                    let uv = [slot.x as f32 / n, slot.y as f32 / n, (slot.x + slot.w) as f32 / n, (slot.y + slot.h) as f32 / n];
                    self.push([whole + slot.ox as f32, base + slot.oy as f32, slot.w as f32, slot.h as f32], [0.0; 4], color, [3.0, 0.0, 0.0, 0.0], uv);
                }
            }
            u += adv;
        }
        u / s
    }

    fn measure_canvas(&mut self, text: &str, face: Face, size: f32) -> f32 {
        self.metrics.measure_canvas(text, face, size, self.scale)
    }

    fn icon(&mut self, name: &str, x: f32, y: f32, size: f32, color: [f32; 4], fill: bool, rotate: f32) {
        let s = self.scale;
        let px = (size * s).round().max(1.0) as u32;
        let n = self.glyphs.shelf.size as f32;
        if let Some(slot) = self.glyphs.icon(name, px, fill, None).map(|s| Slot { ..*s }) {
            let gx = (x * s).round() + slot.ox as f32;
            let gy = (y * s).round() + slot.oy as f32;
            let uv = [slot.x as f32 / n, slot.y as f32 / n, (slot.x + slot.w) as f32 / n, (slot.y + slot.h) as f32 / n];
            self.push([gx, gy, slot.w as f32, slot.h as f32], [0.0; 4], color, [3.0, 0.0, 0.0, rotate], uv);
        }
    }

    fn svg(&mut self, key: &str, source: &str, r: R, alpha: f32) {
        let s = self.scale;
        let (w, h) = ((r.w * s).round().max(1.0) as u32, (r.h * s).round().max(1.0) as u32);
        let k = format!("{key}|{w}x{h}");
        let n = self.images.shelf.size as f32;
        if !self.svgs.contains_key(&k) {
            let placed = render_svg(source, w, h).and_then(|px| {
                let at = match self.images.shelf.alloc(w, h) {
                    Some(a) => a,
                    None => {
                        self.images.shelf.reset();
                        self.images.slots.clear();
                        self.svgs.clear();
                        self.images.shelf.alloc(w, h)?
                    }
                };
                self.images.shelf.put(at, w, h, &px);
                Some((at.0, at.1, w, h))
            });
            self.svgs.insert(k.clone(), placed);
        }
        if let Some(Some((x0, y0, w, h))) = self.svgs.get(&k).copied() {
            let uv = [x0 as f32 / n, y0 as f32 / n, (x0 + w) as f32 / n, (y0 + h) as f32 / n];
            self.push([(r.x * s).round(), (r.y * s).round(), w as f32, h as f32], [0.0; 4], [1.0, 1.0, 1.0, alpha], [4.0, 0.0, 0.0, 0.0], uv);
        }
    }

    fn bitmap(&mut self, key: &str, w: u32, h: u32, rgba: &[u8], r: R, alpha: f32) {
        let k = format!("bitmap|{key}");
        let n = self.images.shelf.size as f32;
        if !self.svgs.contains_key(&k) {
            if rgba.len() != (w * h * 4) as usize || w > self.images.shelf.size || h > self.images.shelf.size {
                return;
            }
            let at = match self.images.shelf.alloc(w, h) {
                Some(a) => Some(a),
                None => {
                    self.images.shelf.reset();
                    self.images.slots.clear();
                    self.svgs.clear();
                    self.images.shelf.alloc(w, h)
                }
            };
            let placed = at.map(|at| {
                self.images.shelf.put(at, w, h, rgba);
                (at.0, at.1, w, h)
            });
            self.svgs.insert(k.clone(), placed);
        }
        if let Some(Some((x0, y0, w, h))) = self.svgs.get(&k).copied() {
            let s = self.scale;
            let uv = [x0 as f32 / n, y0 as f32 / n, (x0 + w) as f32 / n, (y0 + h) as f32 / n];
            self.push(
                [(r.x * s).round(), (r.y * s).round(), (r.w * s).round(), (r.h * s).round()],
                [0.0; 4],
                [1.0, 1.0, 1.0, alpha],
                [4.0, 0.0, 0.0, 0.0],
                uv,
            );
        }
    }

    fn push_clip(&mut self, r: R) {
        let s = self.scale;
        self.clips.push([r.x * s, r.y * s, (r.x + r.w) * s, (r.y + r.h) * s]);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }
}
