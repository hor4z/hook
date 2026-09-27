use super::cff::{div_fix, mul_fix};
use super::fonts::{Face, font, resolve};
use harfrust::{Feature, ShapeOptions, Tag, UnicodeBuffer};
use read_fonts::TableProvider;
use read_fonts::tables::os2::SelectionFlags;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub face: Face,
    pub glyph: u16,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub cluster: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shaped {
    pub glyphs: Vec<Placed>,
    pub advance: f32,
}

pub const UI_FEATURES: [&[u8; 4]; 3] = [b"cv11", b"ss01", b"tnum"];

pub fn advance_px(face: Face, glyph: u16, size: f32) -> f32 {
    let f = font(face);
    let units = f.advance(read_fonts::types::GlyphId16::new(glyph)) as i32;
    let scaled = mul_fix(units, div_fix((size * 64.0) as i32, f.upem as i32));
    ((scaled + 32) & !63) as f32 / 64.0
}

fn runs(face: Face, s: &str) -> Vec<(Face, usize, usize)> {
    let mut out: Vec<(Face, usize, usize)> = Vec::new();
    for (i, ch) in s.char_indices() {
        let f = if ch.is_whitespace() || ch.is_control() {
            out.last().map(|r| r.0).unwrap_or(face)
        } else {
            resolve(face, ch).0
        };
        let end = i + ch.len_utf8();
        match out.last_mut() {
            Some(r) if r.0 == f => r.2 = end,
            _ => out.push((f, i, end)),
        }
    }
    out
}

pub fn shape(face: Face, s: &str, size: f32, tracking: f32) -> Shaped {
    shape_with(face, s, size, tracking, &UI_FEATURES, true)
}

pub fn shape_with(face: Face, s: &str, size: f32, tracking: f32, features: &[&[u8; 4]], kerning: bool) -> Shaped {
    let spacing = tracking * size;
    let mut feats: Vec<Feature> = features.iter().map(|t| Feature::new(Tag::new(t), 1, ..)).collect();
    if !kerning {
        feats.push(Feature::new(Tag::new(b"kern"), 0, ..));
    }
    if spacing != 0.0 {
        for t in [b"liga", b"clig", b"dlig", b"calt"] {
            feats.push(Feature::new(Tag::new(t), 0, ..));
        }
    }
    let mut out = Shaped::default();
    let mut pen = 0.0f32;
    for (f, start, end) in runs(face, s) {
        let fnt = font(f);
        let k = size / fnt.upem;
        let mut buf = UnicodeBuffer::new();
        buf.push_str(&s[start..end]);
        buf.guess_segment_properties();
        let shaped = fnt.shaper.shape(buf, ShapeOptions::new().features(&feats));
        for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            let glyph = info.glyph_id as u16;
            let nominal = fnt.advance(read_fonts::types::GlyphId16::new(glyph)) as i32;
            let advance = (advance_px(f, glyph, size) + (pos.x_advance - nominal) as f32 * k).round() + spacing;
            out.glyphs.push(Placed {
                face: f,
                glyph,
                x: pen + pos.x_offset as f32 * k,
                y: -(pos.y_offset as f32) * k,
                advance,
                cluster: start + info.cluster as usize,
            });
            pen += advance;
        }
    }
    out.advance = pen;
    out
}

pub fn measure(face: Face, s: &str, size: f32, tracking: f32) -> f32 {
    shape(face, s, size, tracking).advance
}

pub fn fit(face: Face, s: &str, size: f32, tracking: f32, max: f32) -> String {
    if measure(face, s, size, tracking) <= max {
        return s.to_string();
    }
    let ell = measure(face, "…", size, tracking);
    let shaped = shape(face, s, size, tracking);
    let mut end = 0;
    for i in 1..=shaped.glyphs.len() {
        let next = shaped.glyphs.get(i);
        if next.map(|n| n.x).unwrap_or(shaped.advance) + ell > max {
            break;
        }
        end = next.map(|n| n.cluster).unwrap_or(s.len());
    }
    let mut out = s[..end].to_string();
    out.push('…');
    out
}

pub fn metrics(face: Face) -> (f32, f32) {
    let f = font(face);
    let typo = f.face.os2().ok().filter(|o| o.fs_selection().contains(SelectionFlags::USE_TYPO_METRICS));
    let (asc, desc) = match typo {
        Some(o) => (o.s_typo_ascender() as f32, o.s_typo_descender() as f32),
        None => {
            let h = f.face.hhea().expect("hhea");
            (h.ascender().to_i16() as f32, h.descender().to_i16() as f32)
        }
    };
    (asc / f.upem, -desc / f.upem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_round_to_the_pixel_like_chromium_on_linux() {
        let s = shape_with(Face::Mono400, "id", 12.0, 0.0, &[], false);
        assert_eq!(s.advance, 14.0);
    }

    #[test]
    fn tracking_adds_after_every_letter() {
        let a = shape(Face::Mono500, "COLUMNAS", 10.5, 0.0).advance;
        let b = shape(Face::Mono500, "COLUMNAS", 10.5, 0.08).advance;
        assert!((b - a - 8.0 * 0.84).abs() < 1e-4);
    }

    #[test]
    fn line_metrics_come_from_the_font() {
        assert_eq!(metrics(Face::Sans400), (0.92, 0.22));
        assert_eq!(metrics(Face::Mono400), (1.005, 0.295));
    }

    #[test]
    fn ss01_and_tnum_change_glyphs() {
        let plain = shape_with(Face::Sans400, "1a", 13.0, 0.0, &[], true);
        let ui = shape(Face::Sans400, "1a", 13.0, 0.0);
        assert_ne!(plain.glyphs.iter().map(|g| g.glyph).collect::<Vec<_>>(), ui.glyphs.iter().map(|g| g.glyph).collect::<Vec<_>>());
    }

    #[test]
    fn fit_leaves_room_for_the_ellipsis() {
        let full = measure(Face::Sans400, "relaciones_de_usuarios", 13.0, 0.0);
        let cut = fit(Face::Sans400, "relaciones_de_usuarios", 13.0, 0.0, full * 0.6);
        assert!(cut.ends_with('…'));
        assert!(measure(Face::Sans400, &cut, 13.0, 0.0) <= full * 0.6 + 0.01);
    }
}
