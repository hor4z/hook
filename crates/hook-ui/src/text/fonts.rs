use super::cff::{Cff, div_fix, mul_fix};
use harfrust::{Shaper, ShaperData};
use read_fonts::tables::cmap::CmapSubtable;
use read_fonts::tables::glyf::Glyph;
use read_fonts::types::{GlyphId16 as GlyphId, Tag};
use read_fonts::{FontRef, TableProvider};
use std::sync::OnceLock;

pub use crate::core::Face;

pub const DATA: [&[u8]; 5] = [
    include_bytes!("../../fonts/GeistSans-Regular.otf"),
    include_bytes!("../../fonts/GeistSans-Medium.otf"),
    include_bytes!("../../fonts/GeistSans-SemiBold.otf"),
    include_bytes!("../../fonts/GeistMono-Regular.ttf"),
    include_bytes!("../../fonts/GeistMono-Medium.ttf"),
];

pub struct Font {
    pub face: FontRef<'static>,
    pub shaper: Shaper<'static>,
    pub upem: f32,
    cmap: Option<CmapSubtable<'static>>,
    cff: Option<Cff<'static>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Default)]
struct Contour {
    out: Vec<Seg>,
    first_on: Option<(f32, f32)>,
    first_off: Option<(f32, f32)>,
    last_off: Option<(f32, f32)>,
}

fn mid(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * 0.5, a.1 + (b.1 - a.1) * 0.5)
}

impl Contour {
    fn quad(&mut self, c: (f32, f32), p: (f32, f32)) {
        self.out.push(Seg::Quad(c.0, c.1, p.0, p.1));
    }

    fn push(&mut self, p: (f32, f32), on: bool, last: bool) {
        if self.first_on.is_none() {
            if on {
                self.first_on = Some(p);
                self.out.push(Seg::Move(p.0, p.1));
            } else if let Some(off) = self.first_off {
                let m = mid(off, p);
                self.first_on = Some(m);
                self.last_off = Some(p);
                self.out.push(Seg::Move(m.0, m.1));
            } else {
                self.first_off = Some(p);
            }
        } else {
            match (self.last_off, on) {
                (Some(off), true) => {
                    self.last_off = None;
                    self.quad(off, p);
                }
                (Some(off), false) => {
                    self.last_off = Some(p);
                    self.quad(off, mid(off, p));
                }
                (None, true) => self.out.push(Seg::Line(p.0, p.1)),
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
            self.quad(b, mid(b, a));
        }
        match (self.first_on, self.first_off, self.last_off) {
            (Some(p), Some(a), _) => self.quad(a, p),
            (Some(p), None, Some(b)) => self.quad(b, p),
            (Some(p), None, None) => self.out.push(Seg::Line(p.0, p.1)),
            _ => {}
        }
        self.first_on = None;
        self.first_off = None;
        self.last_off = None;
        self.out.push(Seg::Close);
    }
}

pub fn font(face: Face) -> &'static Font {
    static ALL: OnceLock<Vec<Font>> = OnceLock::new();
    &ALL.get_or_init(|| {
        DATA.iter()
            .map(|d| {
                let face = FontRef::new(d).expect("invalid font");
                let upem = face.head().expect("head").units_per_em() as f32;
                let cff = face.table_data(Tag::new(b"CFF ")).and_then(|t| Cff::parse(t.as_bytes()));
                let data: &'static ShaperData = Box::leak(Box::new(ShaperData::new(&face)));
                let shaper = data.shaper(&face).build();
                let cmap = face.cmap().ok().and_then(|c| c.best_subtable()).map(|(_, _, t)| t);
                Font { face, shaper, upem, cmap, cff }
            })
            .collect()
    })[face.index()]
}

impl Font {
    pub fn glyph(&self, ch: char) -> Option<GlyphId> {
        let id = self.cmap.as_ref()?.map_codepoint(ch)?;
        GlyphId::try_from(id).ok().filter(|g| g.to_u16() != 0)
    }

    pub fn advance(&self, id: GlyphId) -> f32 {
        self.face.hmtx().ok().and_then(|h| h.advance(id.into())).unwrap_or(0) as f32
    }

    pub fn outline(&self, id: GlyphId) -> Vec<Seg> {
        let mut c = Contour::default();
        let Ok(loca) = self.face.loca(None) else { return c.out };
        let Ok(glyf) = self.face.glyf() else { return c.out };
        let Ok(Some(Glyph::Simple(g))) = loca.get_glyf(id.into(), &glyf) else { return c.out };
        let ends: Vec<usize> = g.end_pts_of_contours().iter().map(|e| e.get() as usize).collect();
        if ends.last() == Some(&0) {
            return c.out;
        }
        for (i, p) in g.points().enumerate() {
            c.push((p.x as f32, p.y as f32), p.on_curve, ends.contains(&i));
        }
        c.out
    }

    pub fn path(&self, id: GlyphId, px: f32, hinted: bool) -> Vec<Seg> {
        let size = (px * 64.0) as i32;
        let scale = div_fix(size, self.upem as i32);
        if let Some(cff) = &self.cff
            && let Some(p) = cff.path(id.to_u16(), (scale + 32) / 64, (scale + 32) / 64, hinted)
        {
            return p;
        }
        let mut out = Vec::new();
        self.tt_path(id, scale, hinted, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0), &mut out, 0);
        out
    }

    fn components(&self, id: GlyphId) -> Option<Vec<(GlyphId, u16, i32, i32, [f32; 4])>> {
        let loca = self.face.table_data(Tag::new(b"loca"))?;
        let loca = loca.as_bytes();
        let glyf = self.face.table_data(Tag::new(b"glyf"))?;
        let glyf = glyf.as_bytes();
        let long = self.face.head().ok()?.index_to_loc_format() == 1;
        let i = id.to_u16() as usize;
        let at = |k: usize| -> Option<usize> {
            if long {
                Some(u32::from_be_bytes(loca.get(k * 4..k * 4 + 4)?.try_into().ok()?) as usize)
            } else {
                Some(u16::from_be_bytes(loca.get(k * 2..k * 2 + 2)?.try_into().ok()?) as usize * 2)
            }
        };
        let (start, end) = (at(i)?, at(i + 1)?);
        let g = glyf.get(start..end)?;
        if g.len() < 10 || i16::from_be_bytes([g[0], g[1]]) >= 0 {
            return None;
        }
        let mut p = 10;
        let u16at = |p: usize| -> Option<u16> { Some(u16::from_be_bytes([*g.get(p)?, *g.get(p + 1)?])) };
        let f2 = |p: usize| -> Option<f32> { Some(i16::from_be_bytes([*g.get(p)?, *g.get(p + 1)?]) as f32 / 16384.0) };
        let mut out = Vec::new();
        loop {
            let flags = u16at(p)?;
            let gid = u16at(p + 2)?;
            p += 4;
            let (dx, dy) = if flags & 1 != 0 {
                let v = (u16at(p)? as i16 as i32, u16at(p + 2)? as i16 as i32);
                p += 4;
                v
            } else {
                let v = (*g.get(p)? as i8 as i32, *g.get(p + 1)? as i8 as i32);
                p += 2;
                v
            };
            let (dx, dy) = if flags & 2 != 0 { (dx, dy) } else { (0, 0) };
            let mut m = [1.0, 0.0, 0.0, 1.0];
            if flags & 8 != 0 {
                let s = f2(p)?;
                m = [s, 0.0, 0.0, s];
                p += 2;
            } else if flags & 0x40 != 0 {
                m = [f2(p)?, 0.0, 0.0, f2(p + 2)?];
                p += 4;
            } else if flags & 0x80 != 0 {
                m = [f2(p)?, f2(p + 2)?, f2(p + 4)?, f2(p + 6)?];
                p += 8;
            }
            out.push((GlyphId::new(gid), flags, dx, dy, m));
            if flags & 0x20 == 0 {
                break;
            }
        }
        Some(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn tt_path(&self, id: GlyphId, scale: i32, hinted: bool, m: [f32; 4], off: (f32, f32), out: &mut Vec<Seg>, depth: u32) {
        if depth < 8
            && let Some(parts) = self.components(id)
        {
            for (gid, flags, dx, dy, cm) in parts {
                let mut ox = mul_fix(dx, scale);
                let mut oy = mul_fix(dy, scale);
                if hinted && flags & 4 != 0 {
                    ox = (ox + 32) & !63;
                    oy = (oy + 32) & !63;
                }
                let nm = [m[0] * cm[0] + m[2] * cm[1], m[1] * cm[0] + m[3] * cm[1], m[0] * cm[2] + m[2] * cm[3], m[1] * cm[2] + m[3] * cm[3]];
                let (px, py) = (ox as f32 / 64.0, oy as f32 / 64.0);
                let no = (off.0 + m[0] * px + m[2] * py, off.1 + m[1] * px + m[3] * py);
                self.tt_path(gid, scale, hinted, nm, no, out, depth + 1);
            }
            return;
        }
        let t = |x: f32, y: f32| {
            let (x, y) = (mul_fix(x.round() as i32, scale) as f32 / 64.0, mul_fix(y.round() as i32, scale) as f32 / 64.0);
            (off.0 + m[0] * x + m[2] * y, off.1 + m[1] * x + m[3] * y)
        };
        for seg in self.outline(id) {
            out.push(match seg {
                Seg::Move(x, y) => {
                    let (x, y) = t(x, y);
                    Seg::Move(x, y)
                }
                Seg::Line(x, y) => {
                    let (x, y) = t(x, y);
                    Seg::Line(x, y)
                }
                Seg::Quad(a, b, x, y) => {
                    let (a, b) = t(a, b);
                    let (x, y) = t(x, y);
                    Seg::Quad(a, b, x, y)
                }
                Seg::Cubic(a, b, c, d, x, y) => {
                    let (a, b) = t(a, b);
                    let (c, d) = t(c, d);
                    let (x, y) = t(x, y);
                    Seg::Cubic(a, b, c, d, x, y)
                }
                Seg::Close => Seg::Close,
            });
        }
    }

    pub fn has(&self, ch: char) -> bool {
        self.glyph(ch).is_some()
    }
}

pub fn resolve(face: Face, ch: char) -> (Face, Option<GlyphId>) {
    let f = font(face);
    if let Some(id) = f.glyph(ch) {
        return (face, Some(id));
    }
    let order: &[Face] = if face.is_mono() { &[Face::Mono400, Face::Sans400] } else { &[Face::Sans400, Face::Mono400] };
    for &other in order {
        if let Some(id) = font(other).glyph(ch) {
            return (other, Some(id));
        }
    }
    (face, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_do_not_take_glyphs_from_the_macroman_table() {
        for c in ['\u{0}', '\u{80}', '\u{9f}'] {
            assert_eq!(font(Face::Sans400).glyph(c), None);
        }
        assert!(font(Face::Sans400).glyph('Ä').is_some());
    }

    #[test]
    fn truetype_outlines_close_every_contour() {
        let f = font(Face::Mono400);
        let segs = f.outline(f.glyph('o').unwrap());
        assert_eq!(segs.iter().filter(|s| matches!(s, Seg::Move(..))).count(), 2);
        assert_eq!(segs.iter().filter(|s| matches!(s, Seg::Close)).count(), 2);
        assert!(segs.iter().any(|s| matches!(s, Seg::Quad(..))));
    }
}
