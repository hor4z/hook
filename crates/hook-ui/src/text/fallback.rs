use super::raster::{Point, Rasterizer, point};
use crate::core::Face;
use read_fonts::ps::cff::CffFontRef;
use read_fonts::ps::cs::CommandSink;
use read_fonts::tables::cmap::{CmapSubtable, PlatformId};
use read_fonts::tables::glyf::Glyph;
use read_fonts::tables::os2::SelectionFlags;
use read_fonts::types::{F2Dot14, Fixed, GlyphId};
use read_fonts::{FileRef, FontRef, TableProvider};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Line(Point, Point),
    Quad(Point, Point, Point),
    Cubic(Point, Point, Point, Point),
}

pub struct Outline {
    pub min: Point,
    pub max: Point,
    pub curves: Vec<Curve>,
}

pub struct Fallback {
    face: FontRef<'static>,
    cmaps: Vec<CmapSubtable<'static>>,
    cff: Option<CffFontRef<'static>>,
    pub upem: f32,
    pub height: f32,
}

#[derive(Clone, Copy)]
struct Affine([f32; 6]);

impl Affine {
    const ID: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    fn then(self, t: Affine) -> Affine {
        let [a1, b1, c1, d1, e1, f1] = self.0;
        let [a2, b2, c2, d2, e2, f2] = t.0;
        Affine([
            a1 * a2 + c1 * b2,
            b1 * a2 + d1 * b2,
            a1 * c2 + c1 * d2,
            b1 * c2 + d1 * d2,
            a1 * e2 + c1 * f2 + e1,
            b1 * e2 + d1 * f2 + f1,
        ])
    }

    fn apply(self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, e, f] = self.0;
        if self.0 == Affine::ID.0 {
            return (x, y);
        }
        (a * x + c * y + e, b * x + d * y + f)
    }
}

struct Bounds {
    min: (f32, f32),
    max: (f32, f32),
}

impl Bounds {
    fn new() -> Bounds {
        Bounds {
            min: (f32::MAX, f32::MAX),
            max: (f32::MIN, f32::MIN),
        }
    }

    fn add(&mut self, x: f32, y: f32) {
        self.min = (self.min.0.min(x), self.min.1.min(y));
        self.max = (self.max.0.max(x), self.max.1.max(y));
    }

    fn empty(&self) -> bool {
        self.min == (f32::MAX, f32::MAX) && self.max == (f32::MIN, f32::MIN)
    }

    fn whole(&self) -> Option<[i16; 4]> {
        let int = |v: f32| -> Option<i16> { if v > -2147483904.0 && v < 2147483648.0 { i16::try_from(v as i32).ok() } else { None } };
        Some([int(self.min.0)?, int(self.min.1)?, int(self.max.0)?, int(self.max.1)?])
    }
}

#[derive(Default)]
struct Pen {
    curves: Vec<Curve>,
    last: Point,
    start: Option<Point>,
}

impl Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.last = point(x, y);
        self.start = Some(self.last);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = point(x, y);
        self.curves.push(Curve::Line(self.last, p));
        self.last = p;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p = point(x, y);
        self.curves.push(Curve::Quad(self.last, point(x1, y1), p));
        self.last = p;
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p = point(x, y);
        self.curves.push(Curve::Cubic(self.last, point(x1, y1), point(x2, y2), p));
        self.last = p;
    }

    fn close(&mut self) {
        if let Some(m) = self.start.take() {
            self.curves.push(Curve::Line(self.last, m));
        }
    }
}

struct Contour<'p> {
    pen: &'p mut Pen,
    bounds: &'p mut Bounds,
    t: Affine,
    first_on: Option<(f32, f32)>,
    first_off: Option<(f32, f32)>,
    last_off: Option<(f32, f32)>,
}

fn mid(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * 0.5, a.1 + (b.1 - a.1) * 0.5)
}

impl Contour<'_> {
    fn at(&mut self, p: (f32, f32)) -> (f32, f32) {
        let q = self.t.apply(p.0, p.1);
        self.bounds.add(q.0, q.1);
        q
    }

    fn move_to(&mut self, p: (f32, f32)) {
        let q = self.at(p);
        self.pen.move_to(q.0, q.1);
    }

    fn line_to(&mut self, p: (f32, f32)) {
        let q = self.at(p);
        self.pen.line_to(q.0, q.1);
    }

    fn quad_to(&mut self, c: (f32, f32), p: (f32, f32)) {
        let c = self.at(c);
        let q = self.at(p);
        self.pen.quad_to(c.0, c.1, q.0, q.1);
    }

    fn push(&mut self, p: (f32, f32), on: bool, last: bool) {
        if self.first_on.is_none() {
            if on {
                self.first_on = Some(p);
                self.move_to(p);
            } else if let Some(off) = self.first_off {
                let m = mid(off, p);
                self.first_on = Some(m);
                self.last_off = Some(p);
                self.move_to(m);
            } else {
                self.first_off = Some(p);
            }
        } else {
            match (self.last_off, on) {
                (Some(off), true) => {
                    self.last_off = None;
                    self.quad_to(off, p);
                }
                (Some(off), false) => {
                    self.last_off = Some(p);
                    self.quad_to(off, mid(off, p));
                }
                (None, true) => self.line_to(p),
                (None, false) => self.last_off = Some(p),
            }
        }
        if last {
            self.close();
        }
    }

    fn close(&mut self) {
        if let (Some(a), Some(b)) = (self.first_off, self.last_off) {
            self.last_off = None;
            self.quad_to(b, mid(b, a));
        }
        match (self.first_on, self.first_off, self.last_off) {
            (Some(p), Some(a), _) => self.quad_to(a, p),
            (Some(p), None, Some(b)) => self.quad_to(b, p),
            (Some(p), None, None) => self.line_to(p),
            _ => {}
        }
        self.first_on = None;
        self.first_off = None;
        self.last_off = None;
        self.pen.close();
    }
}

struct CffSink<'p> {
    pen: &'p mut Pen,
    bounds: &'p mut Bounds,
}

impl CommandSink for CffSink<'_> {
    fn move_to(&mut self, x: Fixed, y: Fixed) {
        let (x, y) = (x.to_f32(), y.to_f32());
        self.bounds.add(x, y);
        self.pen.move_to(x, y);
    }

    fn line_to(&mut self, x: Fixed, y: Fixed) {
        let (x, y) = (x.to_f32(), y.to_f32());
        self.bounds.add(x, y);
        self.pen.line_to(x, y);
    }

    fn curve_to(&mut self, cx0: Fixed, cy0: Fixed, cx1: Fixed, cy1: Fixed, x: Fixed, y: Fixed) {
        let p = [cx0, cy0, cx1, cy1, x, y].map(Fixed::to_f32);
        self.bounds.add(p[0], p[1]);
        self.bounds.add(p[2], p[3]);
        self.bounds.add(p[4], p[5]);
        self.pen.cubic_to(p[0], p[1], p[2], p[3], p[4], p[5]);
    }

    fn close(&mut self) {
        self.pen.close();
    }
}

fn unicode(platform: PlatformId, encoding: u16, table: &CmapSubtable) -> bool {
    match platform {
        PlatformId::Unicode => true,
        PlatformId::Windows if encoding == 1 => true,
        PlatformId::Windows => encoding == 10 && matches!(table, CmapSubtable::Format12(_) | CmapSubtable::Format13(_)),
        _ => false,
    }
}

impl Fallback {
    pub fn load(data: &'static [u8]) -> Option<Fallback> {
        let face = match FileRef::new(data).ok()? {
            FileRef::Font(f) => f,
            FileRef::Collection(c) => c.get(0).ok()?,
        };
        let cmap = face.cmap().ok()?;
        let cmaps = cmap
            .encoding_records()
            .iter()
            .filter_map(|r| {
                let t = r.subtable(cmap.offset_data()).ok()?;
                unicode(r.platform_id(), r.encoding_id(), &t).then_some(t)
            })
            .collect();
        let upem = face.head().ok()?.units_per_em() as f32;
        let (asc, desc) = Fallback::vertical(&face)?;
        let cff = face.table_data(read_fonts::types::Tag::new(b"CFF ")).and_then(|t| CffFontRef::new_cff(t.as_bytes(), 0, None).ok());
        Some(Fallback {
            face,
            cmaps,
            cff,
            upem,
            height: asc - desc,
        })
    }

    fn vertical(face: &FontRef) -> Option<(f32, f32)> {
        let os2 = face.os2().ok();
        if let Some(o) = os2.as_ref().filter(|o| o.fs_selection().contains(SelectionFlags::USE_TYPO_METRICS)) {
            return Some((o.s_typo_ascender() as f32, o.s_typo_descender() as f32));
        }
        let hhea = face.hhea().ok()?;
        let pick = |v: i16, typo: fn(&read_fonts::tables::os2::Os2) -> i16, win: fn(&read_fonts::tables::os2::Os2) -> i16| {
            if v != 0 {
                return v;
            }
            match os2.as_ref() {
                Some(o) if typo(o) != 0 => typo(o),
                Some(o) => win(o),
                None => v,
            }
        };
        let asc = pick(hhea.ascender().to_i16(), |o| o.s_typo_ascender(), |o| o.us_win_ascent() as i16);
        let desc = pick(hhea.descender().to_i16(), |o| o.s_typo_descender(), |o| -(o.us_win_descent() as i16));
        Some((asc as f32, desc as f32))
    }

    pub fn glyph(&self, ch: char) -> u16 {
        self.cmaps.iter().find_map(|t| t.map_codepoint(ch)).map(|g| g.to_u32() as u16).unwrap_or(0)
    }

    pub fn advance(&self, id: u16) -> f32 {
        self.face.hmtx().ok().and_then(|h| h.advance(GlyphId::new(id as u32))).unwrap_or(0) as f32
    }

    pub fn outline(&self, id: u16) -> Option<Outline> {
        let mut pen = Pen::default();
        let mut bounds = Bounds::new();
        if let Some(cff) = &self.cff {
            let gid = GlyphId::new(id as u32);
            let sub = cff.subfont(cff.subfont_index(gid).unwrap_or(0), &[] as &[F2Dot14]).ok()?;
            let mut sink = CffSink { pen: &mut pen, bounds: &mut bounds };
            cff.evaluate_charstring(&sub, gid, &[], &mut sink).ok()?;
        } else {
            let loca = self.face.loca(None).ok()?;
            let glyf = self.face.glyf().ok()?;
            self.tt(&loca, &glyf, id, Affine::ID, 0, &mut pen, &mut bounds)?;
        }
        if bounds.empty() {
            return None;
        }
        let [x_min, y_min, x_max, y_max] = bounds.whole()?;
        if x_min >= x_max || y_min >= y_max {
            return None;
        }
        pen.close();
        Some(Outline {
            min: point(x_min as f32, y_max as f32),
            max: point(x_max as f32, y_min as f32),
            curves: pen.curves,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn tt(&self, loca: &read_fonts::tables::loca::Loca, glyf: &read_fonts::tables::glyf::Glyf, id: u16, t: Affine, depth: u8, pen: &mut Pen, bounds: &mut Bounds) -> Option<()> {
        if depth >= 32 {
            return None;
        }
        match loca.get_glyf(GlyphId::new(id as u32), glyf).ok()? {
            None => Some(()),
            Some(Glyph::Simple(g)) => {
                let ends: Vec<usize> = g.end_pts_of_contours().iter().map(|e| e.get() as usize).collect();
                if ends.last() == Some(&0) {
                    return Some(());
                }
                let mut c = Contour {
                    pen,
                    bounds,
                    t,
                    first_on: None,
                    first_off: None,
                    last_off: None,
                };
                for (i, p) in g.points().enumerate() {
                    c.push((p.x as f32, p.y as f32), p.on_curve, ends.contains(&i));
                }
                Some(())
            }
            Some(Glyph::Composite(g)) => {
                for comp in g.components() {
                    let m = comp.transform;
                    let (e, f) = match comp.anchor {
                        read_fonts::tables::glyf::Anchor::Offset { x, y } => (x as f32, y as f32),
                        read_fonts::tables::glyf::Anchor::Point { .. } => (0.0, 0.0),
                    };
                    let local = Affine([m.xx.to_f32(), m.yx.to_f32(), m.xy.to_f32(), m.yy.to_f32(), e, f]);
                    self.tt(loca, glyf, comp.glyph.to_u16(), t.then(local), depth + 1, pen, bounds)?;
                }
                Some(())
            }
        }
    }
}

pub fn draw(o: &Outline, h: f32, v: f32, off: Point, w: usize, hgt: usize) -> Rasterizer {
    let mut r = Rasterizer::new(w, hgt);
    let up = |p: Point| point(p.x * h + off.x, p.y * -v + off.y);
    for c in &o.curves {
        match *c {
            Curve::Line(a, b) => r.draw_line(up(a), up(b)),
            Curve::Quad(a, b, c) => r.draw_quad(up(a), up(b), up(c)),
            Curve::Cubic(a, b, c, d) => r.draw_cubic(up(a), up(b), up(c), up(d)),
        }
    }
    r
}

pub fn embedded(face: Face) -> &'static Fallback {
    static F: OnceLock<Vec<Fallback>> = OnceLock::new();
    &F.get_or_init(|| crate::text::fonts::DATA.iter().map(|b| Fallback::load(b).expect("invalid font")).collect())[match face {
        Face::Sans400 => 0,
        Face::Sans500 => 1,
        Face::Sans600 => 2,
        Face::Mono400 => 3,
        Face::Mono500 => 4,
    }]
}

fn load(paths: &[&str]) -> Option<Fallback> {
    paths.iter().find_map(|p| Fallback::load(Box::leak(std::fs::read(p).ok()?.into_boxed_slice())))
}

fn system(mono: bool) -> Option<&'static Fallback> {
    static F: OnceLock<[Option<Fallback>; 2]> = OnceLock::new();
    let all = F.get_or_init(|| {
        [
            load(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
                "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
                "/Library/Fonts/Arial Unicode.ttf",
                "C:\\Windows\\Fonts\\seguisym.ttf",
                "C:\\Windows\\Fonts\\segoeui.ttf",
            ]),
            load(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
                "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
                "/System/Library/Fonts/Menlo.ttc",
                "C:\\Windows\\Fonts\\consola.ttf",
            ]),
        ]
    });
    all[if mono { 1 } else { 0 }].as_ref()
}

fn wide() -> Option<&'static Fallback> {
    static F: OnceLock<Option<Fallback>> = OnceLock::new();
    F.get_or_init(|| {
        load(&[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\YuGothR.ttc",
            "C:\\Windows\\Fonts\\malgun.ttf",
        ])
    })
    .as_ref()
}

pub fn pick(face: Face, ch: char) -> &'static Fallback {
    pick_with(face, ch, true)
}

pub fn pick_with(face: Face, ch: char, system_fonts: bool) -> &'static Fallback {
    let f = embedded(face);
    if f.glyph(ch) != 0 || ch == ' ' || !system_fonts {
        return f;
    }
    let mono = matches!(face, Face::Mono400 | Face::Mono500);
    let has = |s: &&'static Fallback| s.glyph(ch) != 0;
    system(mono).filter(has).or_else(|| system(!mono).filter(has)).or_else(|| wide().filter(has)).unwrap_or(f)
}

fn scale_for(f: &Fallback, size: f32) -> f32 {
    let px = size * f.height / f.upem;
    px / f.height
}

fn em(f: &Fallback, size: f32) -> f32 {
    size / f.upem
}

pub struct CharMetrics {
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
    pub left: f32,
    pub right: f32,
}

pub fn measure_char(face: Face, ch: char, size: f32) -> CharMetrics {
    measure_char_with(face, ch, size, true)
}

pub fn measure_char_with(face: Face, ch: char, size: f32, system_fonts: bool) -> CharMetrics {
    let f = pick_with(face, ch, system_fonts);
    let id = f.glyph(ch);
    let k = em(f, size);
    let advance = f.advance(id) * k;
    match f.outline(id) {
        Some(o) => CharMetrics {
            advance,
            ascent: o.min.y.max(o.max.y) * k,
            descent: -o.min.y.min(o.max.y) * k,
            left: -o.min.x * k,
            right: o.max.x * k,
        },
        None => CharMetrics {
            advance,
            ascent: 0.0,
            descent: 0.0,
            left: 0.0,
            right: 0.0,
        },
    }
}

pub fn coverage(face: Face, ch: char, size: f32, ox: f32, baseline: f32, w: usize, h: usize) -> Vec<f32> {
    let f = pick(face, ch);
    raster_id(f, f.glyph(ch), size, ox, baseline, w, h)
}

fn raster_id(f: &Fallback, id: u16, size: f32, ox: f32, baseline: f32, w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    let Some(o) = f.outline(id) else { return out };
    let k = scale_for(f, size);
    let (xt, xf) = (ox.trunc(), ox.fract());
    let (yt, yf) = (baseline.trunc(), baseline.fract());
    let min = point((o.min.x * k + xf).floor() + xt, (o.min.y * -k + yf).floor() + yt);
    let max = point((o.max.x * k + xf).ceil() + xt, (o.max.y * -k + yf).ceil() + yt);
    let (bw, bh) = ((max.x - min.x) as usize, (max.y - min.y) as usize);
    let off = point(ox - min.x, baseline - min.y);
    let bw32 = bw as u32;
    draw(&o, k, k, off, bw, bh).for_each_pixel(|i, c| {
        let (x, y) = (i as u32 % bw32, i as u32 / bw32);
        let px = x as i32 + min.x as i32;
        let py = y as i32 + min.y as i32;
        if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
            let i = py as usize * w + px as usize;
            out[i] = (out[i] + c).min(1.0);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geist() -> Fallback {
        Fallback::load(crate::text::fonts::DATA[3]).unwrap()
    }

    #[test]
    fn bounds_round_toward_zero_and_the_y_axis_is_flipped() {
        let o = geist().outline(geist().glyph('H')).unwrap();
        assert!(o.min.y > o.max.y);
        assert!(o.min.x < o.max.x);
        assert_eq!(o.min.x.fract(), 0.0);
    }

    #[test]
    fn each_contour_ends_with_a_line_back_to_its_start() {
        let o = geist().outline(geist().glyph('o')).unwrap();
        let first = match o.curves[0] {
            Curve::Line(a, _) | Curve::Quad(a, _, _) | Curve::Cubic(a, _, _, _) => a,
        };
        assert!(o.curves.iter().any(|c| matches!(*c, Curve::Line(_, b) if b == first)));
    }

    #[test]
    fn a_missing_glyph_returns_zero() {
        assert_eq!(geist().glyph('\u{21c4}'), 0);
        assert_ne!(geist().glyph('a'), 0);
    }
}
