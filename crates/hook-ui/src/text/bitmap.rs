use super::fonts::{Face, Seg, font, resolve};
use super::raster::{cbox, coverage};
use super::{gamma, icons};
use read_fonts::types::GlyphId16 as GlyphId;

pub const SUB: u8 = 3;
pub const WHITE: usize = gamma::LEVELS - 1;

#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub w: u32,
    pub h: u32,
    pub ox: i32,
    pub oy: i32,
    pub alpha: Vec<u8>,
}

pub const LCD_FILTER: [u8; 5] = [0x08, 0x4d, 0x56, 0x4d, 0x08];

#[derive(Clone, Debug, PartialEq)]
pub struct Lcd {
    pub w: u32,
    pub h: u32,
    pub ox: i32,
    pub oy: i32,
    pub rgb: Vec<[u8; 3]>,
}

impl Lcd {
    pub fn corrected(&self, color: [f32; 3]) -> Vec<[u8; 3]> {
        let t = gamma::tables();
        let lv = color.map(|c| ((c.clamp(0.0, 1.0) * 255.0).round() as usize) >> 5);
        self.rgb.iter().map(|p| [t[lv[0]][p[0] as usize], t[lv[1]][p[1] as usize], t[lv[2]][p[2] as usize]]).collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Raster {
    pub subpixel: bool,
    pub hinting: bool,
    pub level: usize,
}

impl Default for Raster {
    fn default() -> Self {
        Raster {
            subpixel: false,
            hinting: true,
            level: WHITE,
        }
    }
}

impl Raster {
    pub fn glyph(&self, face: Face, ch: char, px: f32, sub: u8) -> Option<Slot> {
        let (face, id) = resolve(face, ch);
        self.glyph_id(face, id?.to_u16(), px, sub)
    }

    pub fn glyph_id(&self, face: Face, id: u16, px: f32, sub: u8) -> Option<Slot> {
        let cov = self.coverage(face, id, px, sub)?;
        Some(Slot {
            alpha: cov.alpha.iter().map(|&c| gamma::tables()[self.level.min(WHITE)][c as usize]).collect(),
            ..cov
        })
    }

    pub fn coverage(&self, face: Face, id: u16, px: f32, sub: u8) -> Option<Slot> {
        if px <= 0.0 {
            return None;
        }
        let segs = font(face).path(GlyphId::new(id), px, self.hinting);
        let b = cbox(&segs).map(|b| b.pixels()).unwrap_or(super::raster::Bounds {
            left: 0.0,
            right: 0.0,
            ascent: 0.0,
            descent: 0.0,
        });
        let frac = sub.min(SUB - 1) as f32 / SUB as f32;
        let shift = if self.subpixel { frac } else { frac.round() };
        let left = (-b.left + frac).floor() as i32 - 1;
        let right = (b.right + frac).ceil() as i32 + 1;
        let up = b.ascent.ceil() as i32 + 1;
        let down = b.descent.ceil() as i32 + 1;
        let w = (right - left).max(1) as u32;
        let h = (up + down).max(1) as u32;
        let cov = coverage(&segs, -left as f32 + shift, up as f32, w as usize, h as usize);
        Some(Slot {
            w,
            h,
            ox: left,
            oy: -up,
            alpha: cov.iter().map(|c| (c * 255.0).round() as u8).collect(),
        })
    }

    pub fn lcd(&self, face: Face, id: u16, px: f32, sub: u8) -> Option<Lcd> {
        if px <= 0.0 {
            return None;
        }
        let segs = font(face).path(GlyphId::new(id), px, self.hinting);
        let b = cbox(&segs).map(|b| b.pixels()).unwrap_or(super::raster::Bounds {
            left: 0.0,
            right: 0.0,
            ascent: 0.0,
            descent: 0.0,
        });
        let frac = sub.min(SUB - 1) as f32 / SUB as f32;
        let shift = if self.subpixel { frac } else { frac.round() };
        let left = (-b.left + frac).floor() as i32 - 2;
        let right = (b.right + frac).ceil() as i32 + 2;
        let up = b.ascent.ceil() as i32 + 1;
        let down = b.descent.ceil() as i32 + 1;
        let w = (right - left).max(1) as usize;
        let h = (up + down).max(1) as usize;
        let wide: Vec<Seg> = segs
            .iter()
            .map(|s| match *s {
                Seg::Move(x, y) => Seg::Move(x * 3.0, y),
                Seg::Line(x, y) => Seg::Line(x * 3.0, y),
                Seg::Quad(a, b, x, y) => Seg::Quad(a * 3.0, b, x * 3.0, y),
                Seg::Cubic(a, b, c, d, x, y) => Seg::Cubic(a * 3.0, b, c * 3.0, d, x * 3.0, y),
                Seg::Close => Seg::Close,
            })
            .collect();
        let cov = coverage(&wide, (-left as f32 + shift) * 3.0, up as f32, w * 3, h);
        let mut rgb = vec![[0u8; 3]; w * h];
        for y in 0..h {
            let row = &cov[y * w * 3..(y + 1) * w * 3];
            for x in 0..w * 3 {
                let mut v = 0.0;
                for (k, wt) in LCD_FILTER.iter().enumerate() {
                    let i = x as i32 + k as i32 - 2;
                    if i >= 0 && (i as usize) < w * 3 {
                        v += row[i as usize] * *wt as f32;
                    }
                }
                rgb[y * w + x / 3][x % 3] = (v / 256.0 * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
        Some(Lcd {
            w: w as u32,
            h: h as u32,
            ox: left,
            oy: -up,
            rgb,
        })
    }

    pub fn icon(&self, name: &str, px: u32, fill: bool, stroke: f32) -> Option<Slot> {
        let icon = icons::get(name)?;
        let w = px + 2;
        let alpha = icons::render(icon, w, w, 1.0, 1.0, px as f32 / 24.0, stroke, fill);
        Some(Slot { w, h: w, ox: -1, oy: -1, alpha })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_at_its_exact_size_has_the_browser_box() {
        let s = Raster::default().glyph(Face::Sans400, 'H', 13.0, 0).unwrap();
        assert_eq!((s.w, s.h, s.ox, s.oy), (9, 12, 0, -11));
        assert!((s.alpha[9 + 1] as i32 - 215).abs() <= 2);
    }

    #[test]
    fn without_subpixel_phase_two_shifts_the_glyph_one_pixel() {
        let r = Raster::default();
        let a = r.glyph(Face::Sans400, 'H', 13.0, 0).unwrap();
        let c = r.glyph(Face::Sans400, 'H', 13.0, 2).unwrap();
        assert_eq!(c.w, a.w + 1);
        assert_eq!(c.alpha[5 * c.w as usize + 2], a.alpha[5 * a.w as usize + 1]);
    }

    #[test]
    fn lcd_spreads_coverage_over_three_channels() {
        let r = Raster::default();
        let g = r.coverage(Face::Sans400, 43, 13.0, 0).unwrap();
        let l = r.lcd(Face::Sans400, 43, 13.0, 0).unwrap();
        assert_eq!((l.w, l.ox, l.h, l.oy), (g.w + 2, g.ox - 1, g.h, g.oy));
        assert!(l.rgb.iter().any(|p| p[0] != p[2]));
        let gray: u32 = g.alpha.iter().map(|&a| a as u32).sum();
        let lcd: u32 = l.rgb.iter().map(|p| p[1] as u32).sum();
        assert!((gray as i64 - lcd as i64).abs() * 20 < gray as i64);
    }
}
