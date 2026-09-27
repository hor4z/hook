use super::fonts::{Face, font, resolve};
use super::raster::{Bounds, cbox};

pub struct Metrics {
    pub advance: f64,
    pub bounds: Option<Bounds>,
}

pub fn measure(face: Face, ch: char, px: f32) -> (Face, Option<read_fonts::types::GlyphId16>, Metrics) {
    let (face, id) = resolve(face, ch);
    let f = font(face);
    let Some(id) = id else {
        return (face, None, Metrics { advance: 0.0, bounds: None });
    };
    let adv = super::cff::mul_fix(f.advance(id) as i32, super::cff::div_fix((px * 64.0) as i32, f.upem as i32));
    let advance = ((adv + 32) & !63) as f64 / 64.0;
    let bounds = cbox(&f.path(id, px, true)).map(|b| b.pixels());
    (face, Some(id), Metrics { advance, bounds })
}
