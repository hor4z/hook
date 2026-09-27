use std::sync::OnceLock;

pub const CONTRAST: f32 = 0.2;
pub const GAMMA: f32 = 1.2;
pub const LEVELS: usize = 8;

fn scale255(base: u32) -> u32 {
    (base << 5) | (base << 2) | (base >> 1)
}

pub fn level(rgb: [f32; 3]) -> usize {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let lum = (b(rgb[0]) * 54 + b(rgb[1]) * 183 + b(rgb[2]) * 19) >> 8;
    (lum >> 5) as usize
}

fn build(lum: u32) -> [u8; 256] {
    let mut table = [0u8; 256];
    let src = lum as f32 / 255.0;
    let lin_src = src.powf(GAMMA);
    let dst = 1.0 - src;
    let lin_dst = dst.powf(GAMMA);
    let contrast = CONTRAST * lin_dst;
    for (i, t) in table.iter_mut().enumerate() {
        let raw = i as f32 / 255.0;
        let a = raw + (1.0 - raw) * contrast * raw;
        if (src - dst).abs() < 1.0 / 256.0 {
            *t = (255.0 * a).round() as u8;
        } else {
            let lin_out = lin_src * a + (1.0 - a) * lin_dst;
            let out = lin_out.powf(1.0 / GAMMA);
            *t = (255.0 * (out - dst) / (src - dst)).round().clamp(0.0, 255.0) as u8;
        }
    }
    table
}

pub fn tables() -> &'static [[u8; 256]; LEVELS] {
    static T: OnceLock<[[u8; 256]; LEVELS]> = OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|i| build(scale255(i as u32))))
}

pub fn apply(level: usize, coverage: f32) -> u8 {
    tables()[level.min(LEVELS - 1)][(coverage.clamp(0.0, 1.0) * 255.0).round() as usize]
}
