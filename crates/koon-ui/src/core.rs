pub use crate::semantics::{Describe, Node, Role, States, Value, ValueRef};

use std::collections::HashMap;

pub type Rgba = [f32; 4];

pub fn hex(value: &str, alpha: f32) -> Rgba {
    let n = u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap_or(0);
    [((n >> 16) & 255) as f32 / 255.0, ((n >> 8) & 255) as f32 / 255.0, (n & 255) as f32 / 255.0, alpha]
}

pub fn with_alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], a]
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct R {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl R {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> R {
        R { x, y, w, h }
    }
    pub fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }
    pub fn inset(&self, d: f32) -> R {
        R::new(self.x + d, self.y + d, self.w - 2.0 * d, self.h - 2.0 * d)
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn intersect(&self, o: &R) -> R {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        R::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Sans400,
    Sans500,
    Sans600,
    Mono400,
    Mono500,
}

impl Face {
    pub const ALL: [Face; 5] = [Face::Sans400, Face::Sans500, Face::Sans600, Face::Mono400, Face::Mono500];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn is_mono(self) -> bool {
        matches!(self, Face::Mono400 | Face::Mono500)
    }
}

pub trait Painter {
    fn rect(&mut self, r: R, radii: [f32; 4], fill: Rgba);
    fn border(&mut self, r: R, radii: [f32; 4], color: Rgba, width: f32);
    fn gradient(&mut self, r: R, radii: [f32; 4], top: Rgba, _bottom: Rgba) {
        self.rect(r, radii, top);
    }
    fn shadow(&mut self, r: R, radius: f32, blur: f32, spread: f32, dy: f32, color: Rgba);
    fn segment(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: Rgba);
    fn segment_butt(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: Rgba) {
        self.segment(a, b, width, color);
    }
    fn arc(&mut self, c: (f32, f32), r: f32, start: f32, sweep: f32, width: f32, color: Rgba) {
        let n = (sweep / std::f32::consts::TAU * 48.0).ceil().max(1.0) as usize;
        let at = |a: f32| (c.0 + r * a.cos(), c.1 + r * a.sin());
        for k in 0..n {
            let a = start + sweep * k as f32 / n as f32;
            self.segment(at(a), at(a + sweep / n as f32), width, color);
        }
    }
    fn text(&mut self, x: f32, baseline: f32, s: &str, face: Face, size: f32, color: Rgba, tracking: f32) -> f32;
    fn measure(&mut self, s: &str, face: Face, size: f32, tracking: f32) -> f32;
    fn metrics(&mut self, face: Face) -> (f32, f32);
    fn text_canvas(&mut self, x: f32, baseline: f32, s: &str, face: Face, size: f32, color: Rgba) -> f32;
    fn measure_canvas(&mut self, s: &str, face: Face, size: f32) -> f32;
    fn icon(&mut self, name: &str, x: f32, y: f32, size: f32, color: Rgba, fill: bool, rotate: f32);
    fn svg(&mut self, key: &str, source: &str, r: R, alpha: f32);
    fn bitmap(&mut self, _key: &str, _w: u32, _h: u32, _rgba: &[u8], _r: R, _alpha: f32) {}
    fn push_clip(&mut self, r: R);
    fn pop_clip(&mut self);
}

#[derive(Clone, Debug, PartialEq)]
pub struct Css {
    pub dark: bool,
    pub canvas: Rgba,
    pub paper: Rgba,
    pub raised: Rgba,
    pub soft: Rgba,
    pub hover: Rgba,
    pub line: Rgba,
    pub strong: Rgba,
    pub ink: Rgba,
    pub muted: Rgba,
    pub faint: Rgba,
    pub primary: Rgba,
    pub on_primary: Rgba,
    pub brand: Rgba,
    pub accent: Rgba,
    pub danger: Rgba,
    pub warning: Rgba,
    pub success: Rgba,
    pub info: Rgba,
}

fn h(v: &str) -> Rgba {
    hex(v, 1.0)
}

impl Css {
    pub fn dark() -> Css {
        Css {
            dark: true,
            canvas: h("#101112"),
            paper: h("#16171a"),
            raised: h("#1b1c20"),
            soft: h("#1f2125"),
            hover: h("#26282c"),
            line: h("#25272b"),
            strong: h("#33363b"),
            ink: h("#edf2f5"),
            muted: h("#aab3bd"),
            faint: h("#6c757e"),
            primary: h("#edf2f5"),
            on_primary: h("#101112"),
            brand: h("#18a8f0"),
            accent: h("#ff7100"),
            danger: h("#ff3030"),
            warning: h("#ff7100"),
            success: h("#7cbf6b"),
            info: h("#18a8f0"),
        }
    }
    pub fn light() -> Css {
        Css {
            dark: false,
            canvas: h("#fafafa"),
            paper: h("#ffffff"),
            raised: h("#ffffff"),
            soft: h("#f5f5f5"),
            hover: h("#efefef"),
            line: h("#e5e5e5"),
            strong: h("#d4d4d4"),
            ink: h("#0a0a0a"),
            muted: h("#737373"),
            faint: h("#a3a3a3"),
            primary: h("#171717"),
            on_primary: h("#fafafa"),
            brand: h("#5b93a6"),
            accent: h("#dd5536"),
            danger: h("#dd3a2a"),
            warning: h("#d98a2b"),
            success: h("#8aa344"),
            info: h("#5b93a6"),
        }
    }
}

pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

pub fn alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], c[3] * a]
}

pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let bx = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * x1 + 3.0 * (1.0 - s) * s * s * x2 + s * s * s;
    let by = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * y1 + 3.0 * (1.0 - s) * s * s * y2 + s * s * s;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut s = t;
    for _ in 0..24 {
        s = (lo + hi) / 2.0;
        if bx(s) < t {
            lo = s;
        } else {
            hi = s;
        }
    }
    by(s)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    Css,
    Snap,
    Fold,
    OutCubic,
    Bezier(f32, f32, f32, f32),
}

impl Ease {
    pub fn at(self, t: f32) -> f32 {
        match self {
            Ease::Linear => t.clamp(0.0, 1.0),
            Ease::Css => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            Ease::Snap => cubic_bezier(0.2, 0.9, 0.1, 1.0, t),
            Ease::Fold => cubic_bezier(0.33, 0.9, 0.4, 1.0, t),
            Ease::OutCubic => 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3),
            Ease::Bezier(a, b, c, d) => cubic_bezier(a, b, c, d, t),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Anim {
    from: f32,
    to: f32,
    start: f64,
    duration: f64,
    delay: f64,
    ease: Ease,
}

impl Anim {
    fn value(&self, now: f64) -> f32 {
        let t = if self.duration <= 0.0 {
            1.0
        } else {
            ((now - self.start - self.delay) / self.duration).clamp(0.0, 1.0) as f32
        };
        self.from + (self.to - self.from) * self.ease.at(t)
    }
    fn done(&self, now: f64) -> bool {
        now >= self.start + self.delay + self.duration
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    Tab,
    PageUp,
    PageDown,
    Char(char),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub mouse: (f32, f32),
    pub inside: bool,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub right_pressed: bool,
    pub double: bool,
    pub wheel: (f32, f32),
    pub keys: Vec<(Key, Mods)>,
    pub text: String,
    pub mods: Mods,
    pub paste: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    Crosshair,
    Alias,
}

#[derive(Clone, Debug, Default)]
pub struct Field {
    pub caret: usize,
    pub anchor: usize,
    pub scroll: f32,
}

#[derive(Default)]
pub struct State {
    anims: HashMap<u64, Anim>,
    regions: Vec<(R, u64)>,
    prev: Vec<(R, u64)>,
    pub hot: Option<u64>,
    pub active: Option<u64>,
    pub focus: Option<u64>,
    pub fields: HashMap<u64, Field>,
    pub scroll: HashMap<u64, f32>,
    scroll_target: HashMap<u64, f32>,
    grab: f32,
    pub animating: bool,
    pub cursor: Cursor,
    pub consumed_keys: bool,
    pub tooltip: Option<(u64, String, f64)>,
    pub copy_out: Option<String>,
    semantics: bool,
    nodes: Vec<Node>,
    tree: Vec<Node>,
    stack: Vec<usize>,
    interactive_now: Vec<u64>,
    interactive: Vec<u64>,
}

impl State {
    pub fn hit(&self, p: (f32, f32)) -> Option<u64> {
        self.regions.iter().rev().find(|(r, _)| r.contains(p)).map(|(_, i)| *i)
    }

    pub fn regions(&self) -> &[(R, u64)] {
        &self.regions
    }

    pub fn with_semantics(mut self) -> State {
        self.semantics = true;
        self
    }

    pub fn set_semantics(&mut self, on: bool) {
        self.semantics = on;
    }

    pub fn semantics(&self) -> bool {
        self.semantics
    }

    pub fn tree(&self) -> &[Node] {
        &self.tree
    }

    pub fn interactive(&self) -> &[u64] {
        &self.interactive
    }
}

pub fn id(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in parts {
        for b in p.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub struct Ui<'a> {
    pub p: &'a mut dyn Painter,
    pub input: &'a Input,
    pub s: &'a mut State,
    pub css: &'a Css,
    pub now: f64,
    pub width: f32,
    pub height: f32,
    alphas: Vec<f32>,
    offsets: Vec<(f32, f32)>,
    clips: Vec<R>,
}

impl<'a> Ui<'a> {
    pub fn begin(p: &'a mut dyn Painter, input: &'a Input, s: &'a mut State, css: &'a Css, now: f64, width: f32, height: f32) -> Ui<'a> {
        s.prev = std::mem::take(&mut s.regions);
        s.hot = if input.inside {
            s.prev.iter().rev().find(|(r, _)| r.contains(input.mouse)).map(|(_, i)| *i)
        } else {
            None
        };
        if input.pressed {
            let hit = s.hot;
            s.focus = s.focus.filter(|f| hit == Some(*f));
        }
        s.animating = false;
        s.cursor = Cursor::Default;
        s.consumed_keys = false;
        if s.semantics {
            s.nodes.clear();
            s.stack.clear();
            s.interactive_now.clear();
        }
        Ui {
            p,
            input,
            s,
            css,
            now,
            width,
            height,
            alphas: vec![1.0],
            offsets: vec![(0.0, 0.0)],
            clips: Vec::new(),
        }
    }

    pub fn end(self) {
        if self.s.semantics {
            std::mem::swap(&mut self.s.nodes, &mut self.s.tree);
            std::mem::swap(&mut self.s.interactive_now, &mut self.s.interactive);
        }
        if self.input.released {
            self.s.active = None;
        }
        let now = self.now;
        self.s.anims.retain(|_, a| !a.done(now - 5000.0) || a.to != 0.0);
    }

    pub fn a(&self) -> f32 {
        *self.alphas.last().unwrap()
    }

    fn off(&self) -> (f32, f32) {
        *self.offsets.last().unwrap()
    }

    pub fn push_alpha(&mut self, a: f32) {
        let v = self.a() * a;
        self.alphas.push(v);
    }

    pub fn pop_alpha(&mut self) {
        self.alphas.pop();
    }

    pub fn push_offset(&mut self, dx: f32, dy: f32) {
        let (x, y) = self.off();
        self.offsets.push((x + dx, y + dy));
    }

    pub fn pop_offset(&mut self) {
        self.offsets.pop();
    }

    pub fn map(&self, r: R) -> R {
        let (x, y) = self.off();
        R::new(r.x + x, r.y + y, r.w, r.h)
    }

    pub fn region(&mut self, r: R, id: u64) {
        let mut m = self.map(r);
        if let Some(c) = self.clips.last() {
            m = m.intersect(c);
        }
        if m.w > 0.0 && m.h > 0.0 && self.a() > 0.05 {
            self.s.regions.push((m, id));
        }
    }

    pub fn hovered(&self, id: u64) -> bool {
        self.s.hot == Some(id)
    }

    pub fn over_ui(&self) -> bool {
        self.s.hot.is_some()
    }

    pub fn anim(&mut self, key: u64, target: f32, duration: f64, ease: Ease) -> f32 {
        self.anim_delay(key, target, duration, 0.0, ease)
    }

    pub fn anim_delay(&mut self, key: u64, target: f32, duration: f64, delay: f64, ease: Ease) -> f32 {
        let now = self.now;
        let a = self.s.anims.entry(key).or_insert(Anim {
            from: target,
            to: target,
            start: now,
            duration,
            delay,
            ease,
        });
        if a.to != target {
            let current = a.value(now);
            *a = Anim {
                from: current,
                to: target,
                start: now,
                duration,
                delay,
                ease,
            };
        }
        let v = a.value(now);
        if !a.done(now) {
            self.s.animating = true;
        }
        v
    }

    pub fn appear(&mut self, key: u64, duration: f64, delay: f64, ease: Ease) -> f32 {
        let now = self.now;
        self.s.anims.entry(key).or_insert(Anim {
            from: 0.0,
            to: 1.0,
            start: now,
            duration,
            delay,
            ease,
        });
        self.anim_delay(key, 1.0, duration, delay, ease)
    }

    pub fn set_anim(&mut self, key: u64, value: f32) {
        self.s.anims.insert(
            key,
            Anim {
                from: value,
                to: value,
                start: self.now,
                duration: 0.0,
                delay: 0.0,
                ease: Ease::Linear,
            },
        );
    }

    pub fn rect(&mut self, r: R, radius: f32, fill: Rgba) {
        self.rect_radii(r, [radius; 4], fill);
    }

    pub fn gradient_smooth(&mut self, r: R, radius: f32, top: Rgba, bottom: Rgba) {
        if top[3] * self.a() <= 0.001 {
            return;
        }
        let m = self.map(r);
        let a = self.a();
        self.p.gradient(m, [-radius; 4], alpha(top, a), bottom);
    }

    pub fn rect_smooth(&mut self, r: R, radius: f32, fill: Rgba) {
        self.rect_radii(r, [-radius; 4], fill);
    }

    pub fn rect_radii(&mut self, r: R, radii: [f32; 4], fill: Rgba) {
        if fill[3] * self.a() <= 0.001 {
            return;
        }
        let m = self.map(r);
        let a = self.a();
        self.p.rect(m, radii, alpha(fill, a));
    }

    pub fn border(&mut self, r: R, radius: f32, color: Rgba, width: f32) {
        self.border_radii(r, [radius; 4], color, width);
    }

    pub fn border_radii(&mut self, r: R, radii: [f32; 4], color: Rgba, width: f32) {
        if color[3] * self.a() <= 0.001 {
            return;
        }
        let m = self.map(r);
        let a = self.a();
        self.p.border(m, radii, alpha(color, a), width);
    }

    pub fn shadow(&mut self, r: R, radius: f32, blur: f32, spread: f32, dy: f32, color: Rgba) {
        let m = self.map(r);
        let a = self.a();
        self.p.shadow(m, radius, blur, spread, dy, alpha(color, a));
    }

    pub fn segment(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: Rgba) {
        let (x, y) = self.off();
        let al = self.a();
        self.p.segment((a.0 + x, a.1 + y), (b.0 + x, b.1 + y), width, alpha(color, al));
    }

    pub fn segment_butt(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: Rgba) {
        let (x, y) = self.off();
        let al = self.a();
        self.p.segment_butt((a.0 + x, a.1 + y), (b.0 + x, b.1 + y), width, alpha(color, al));
    }

    pub fn arc(&mut self, c: (f32, f32), r: f32, start: f32, sweep: f32, width: f32, color: Rgba) {
        let (x, y) = self.off();
        let al = self.a();
        self.p.arc((c.0 + x, c.1 + y), r, start, sweep, width, alpha(color, al));
    }

    pub fn text(&mut self, x: f32, baseline: f32, s: &str, face: Face, size: f32, color: Rgba) -> f32 {
        self.text_t(x, baseline, s, face, size, color, 0.0)
    }

    pub fn text_t(&mut self, x: f32, baseline: f32, s: &str, face: Face, size: f32, color: Rgba, tracking: f32) -> f32 {
        let (ox, oy) = self.off();
        let a = self.a();
        self.p.text(x + ox, baseline + oy, s, face, size, alpha(color, a), tracking)
    }

    pub fn text_c(&mut self, x: f32, baseline: f32, s: &str, face: Face, size: f32, color: Rgba) -> f32 {
        let (ox, oy) = self.off();
        let a = self.a();
        self.p.text_canvas(x + ox, baseline + oy, s, face, size, alpha(color, a)) - ox
    }

    pub fn measure_c(&mut self, s: &str, face: Face, size: f32) -> f32 {
        self.p.measure_canvas(s, face, size)
    }

    pub fn fit_c(&mut self, s: &str, face: Face, size: f32, max: f32) -> String {
        if self.p.measure_canvas(s, face, size) <= max {
            return s.to_string();
        }
        let ell = self.p.measure_canvas("…", face, size);
        let mut w = 0.0;
        let mut out = String::new();
        for ch in s.chars() {
            let a = self.p.measure_canvas(&ch.to_string(), face, size);
            if w + a + ell > max {
                break;
            }
            w += a;
            out.push(ch);
        }
        out + "…"
    }

    pub fn measure(&mut self, s: &str, face: Face, size: f32) -> f32 {
        self.p.measure(s, face, size, 0.0)
    }

    pub fn measure_t(&mut self, s: &str, face: Face, size: f32, tracking: f32) -> f32 {
        self.p.measure(s, face, size, tracking)
    }

    pub fn baseline(&mut self, face: Face, size: f32, cy: f32) -> f32 {
        let (a, d) = self.p.metrics(face);
        cy + ((a * size).round() - (d * size).round()) / 2.0
    }

    pub fn centered(&mut self, x: f32, cy: f32, s: &str, face: Face, size: f32, color: Rgba) -> f32 {
        let b = self.baseline(face, size, cy);
        self.text(x, b, s, face, size, color)
    }

    pub fn fit(&mut self, s: &str, face: Face, size: f32, max: f32) -> String {
        self.fit_t(s, face, size, max, 0.0)
    }

    pub fn fit_t(&mut self, s: &str, face: Face, size: f32, max: f32, tracking: f32) -> String {
        if self.p.measure(s, face, size, tracking) <= max {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate: String = chars[..mid].iter().collect::<String>() + "…";
            if self.p.measure(&candidate, face, size, tracking) <= max {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        chars[..lo].iter().collect::<String>() + "…"
    }

    pub fn truncate(&mut self, x: f32, baseline: f32, s: &str, face: Face, size: f32, color: Rgba, max: f32) -> f32 {
        let t = self.fit(s, face, size, max);
        self.text(x, baseline, &t, face, size, color)
    }

    pub fn icon(&mut self, name: &str, x: f32, y: f32, size: f32, color: Rgba) {
        self.icon_full(name, x, y, size, color, false, 0.0);
    }

    pub fn icon_full(&mut self, name: &str, x: f32, y: f32, size: f32, color: Rgba, fill: bool, rotate: f32) {
        let (ox, oy) = self.off();
        let a = self.a();
        self.p.icon(name, x + ox, y + oy, size, alpha(color, a), fill, rotate);
    }

    pub fn svg(&mut self, key: &str, source: &str, r: R) {
        let m = self.map(r);
        let a = self.a();
        self.p.svg(key, source, m, a);
    }

    pub fn bitmap(&mut self, key: &str, w: u32, h: u32, rgba: &[u8], r: R) {
        let m = self.map(r);
        let a = self.a();
        self.p.bitmap(key, w, h, rgba, m, a);
    }

    pub fn clip(&mut self, r: R) {
        let mut m = self.map(r);
        if let Some(c) = self.clips.last() {
            m = m.intersect(c);
        }
        self.clips.push(m);
        self.p.push_clip(m);
    }

    pub fn unclip(&mut self) {
        self.clips.pop();
        self.p.pop_clip();
    }

    pub fn describing(&self) -> bool {
        self.s.semantics
    }

    pub fn describe(&mut self, d: Describe) -> Option<usize> {
        if !self.s.semantics {
            return None;
        }
        let mut m = self.map(d.rect);
        if let Some(c) = self.clips.last() {
            m = m.intersect(c);
        }
        let states = d
            .states
            .with(States::HOVERED, self.s.hot == Some(d.id))
            .with(States::FOCUSED, self.s.focus == Some(d.id))
            .with(States::PRESSED, self.s.active == Some(d.id))
            .with(States::HIDDEN, m.w <= 0.0 || m.h <= 0.0 || self.a() <= 0.05);
        self.s.nodes.push(Node {
            id: d.id,
            parent: self.s.stack.last().copied(),
            role: d.role,
            name: d.name.to_string(),
            value: d.value.map(ValueRef::owned),
            states,
            rect: m,
        });
        Some(self.s.nodes.len() - 1)
    }

    pub fn open(&mut self, d: Describe) {
        if let Some(i) = self.describe(d) {
            self.s.stack.push(i);
        }
    }

    pub fn close(&mut self) {
        if self.s.semantics {
            self.s.stack.pop();
        }
    }

    pub fn name(&mut self, id: u64, name: &str) {
        if let Some(n) = self.s.nodes.iter_mut().rev().find(|n| n.id == id) {
            n.name = name.to_string();
        }
    }

    pub fn clicked(&mut self, id: u64) -> bool {
        if self.s.semantics {
            self.s.interactive_now.push(id);
        }
        if self.hovered(id) && self.input.pressed {
            self.s.active = Some(id);
        }
        self.input.released && self.s.active == Some(id) && self.hovered(id)
    }

    pub fn pressed_on(&self, id: u64) -> bool {
        self.input.pressed && self.hovered(id)
    }

    pub fn is_active(&self, id: u64) -> bool {
        self.s.active == Some(id)
    }

    pub fn cursor(&mut self, c: Cursor) {
        self.s.cursor = c;
    }

    pub fn key(&mut self, k: Key) -> Option<Mods> {
        self.input.keys.iter().find(|(x, _)| *x == k).map(|(_, m)| *m)
    }

    pub fn consume_keys(&mut self) {
        self.s.consumed_keys = true;
    }

    pub fn scroll_offset(&mut self, key: u64) -> f32 {
        *self.s.scroll.get(&key).unwrap_or(&0.0)
    }

    pub fn scroll(&mut self, key: u64, view: R, content: f32) -> f32 {
        if self.s.semantics {
            self.s.interactive_now.push(key);
            self.describe(Describe::new(key, Role::ScrollArea, view));
        }
        let max = (content - view.h).max(0.0);
        let hovered = self.map(view).contains(self.input.mouse) && self.input.inside;
        let target = *self.s.scroll_target.get(&key).unwrap_or(&0.0);
        let mut next = target;
        if hovered && self.input.wheel.1 != 0.0 && !self.s.consumed_keys {
            next = (target - self.input.wheel.1).clamp(0.0, max);
        }
        next = next.clamp(0.0, max);
        self.s.scroll_target.insert(key, next);
        let cur = *self.s.scroll.get(&key).unwrap_or(&0.0);
        let v = if (next - cur).abs() < 0.5 { next } else { cur + (next - cur) * 0.35 };
        if v != next {
            self.s.animating = true;
        }
        self.s.scroll.insert(key, v);
        v
    }

    pub fn scroll_to(&mut self, key: u64, y: f32) {
        self.s.scroll_target.insert(key, y.max(0.0));
    }

    pub fn scroll_target(&self, key: u64) -> f32 {
        *self.s.scroll_target.get(&key).unwrap_or(&0.0)
    }

    pub fn scrollbar(&mut self, key: u64, view: R, content: f32, offset: f32) {
        if content <= view.h + 0.5 {
            return;
        }
        let max = content - view.h;
        let h = (view.h * view.h / content).max(24.0);
        let travel = (view.h - h).max(1.0);
        let bar = key ^ 0x5c20_11ba_7a11_0000;
        self.region(R::new(view.right() - 10.0, view.y, 10.0, view.h), bar);
        let mut offset = offset;
        let mouse = self.input.mouse.1 - self.map(view).y;
        if self.pressed_on(bar) {
            let top = travel * offset / max;
            let grab = if mouse >= top && mouse <= top + h { mouse - top } else { h / 2.0 };
            self.s.active = Some(bar);
            self.s.grab = grab;
        }
        if self.is_active(bar) && (self.input.down || self.input.pressed) {
            offset = ((mouse - self.s.grab) / travel * max).clamp(0.0, max);
            self.s.scroll.insert(key, offset);
            self.s.scroll_target.insert(key, offset);
        }
        if self.hovered(bar) || self.is_active(bar) {
            self.cursor(if self.is_active(bar) { Cursor::Grabbing } else { Cursor::Grab });
        }
        let y = view.y + travel * (offset / max);
        let c = if self.is_active(bar) { self.css.strong } else { self.css.line };
        self.rect(R::new(view.right() - 7.0, y + 2.0, 5.0, h - 4.0), 2.5, c);
    }

    pub fn tooltip(&mut self, key: u64, text: &str) {
        if !self.hovered(key) {
            if self.s.tooltip.as_ref().is_some_and(|(k, _, _)| *k == key) {
                self.s.tooltip = None;
            }
            return;
        }
        match &self.s.tooltip {
            Some((k, _, _)) if *k == key => {}
            _ => self.s.tooltip = Some((key, text.to_string(), self.now)),
        }
    }
}
