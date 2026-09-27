use crate::Image;

pub struct Edges {
    w: u32,
    h: u32,
    on: Vec<bool>,
    sum_x: Vec<u32>,
    sum_y: Vec<u32>,
}

impl Edges {
    pub fn new(img: &Image) -> Edges {
        let (w, h) = (img.w, img.h);
        let px = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            [img.rgba[i] as i32, img.rgba[i + 1] as i32, img.rgba[i + 2] as i32]
        };
        let diff = |a: [i32; 3], b: [i32; 3]| (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs();
        let mut on = vec![false; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let c = px(x, y);
                let right = if x + 1 < w { diff(c, px(x + 1, y)) } else { 0 };
                let down = if y + 1 < h { diff(c, px(x, y + 1)) } else { 0 };
                on[(y * w + x) as usize] = right.max(down) > 24;
            }
        }
        let mut sum_x = vec![0u32; ((w + 1) * h) as usize];
        let mut sum_y = vec![0u32; (w * (h + 1)) as usize];
        for y in 0..h {
            for x in 0..w {
                let e = on[(y * w + x) as usize] as u32;
                sum_x[(y * (w + 1) + x + 1) as usize] = sum_x[(y * (w + 1) + x) as usize] + e;
                sum_y[((y + 1) * w + x) as usize] = sum_y[(y * w + x) as usize] + e;
            }
        }
        Edges { w, h, on, sum_x, sum_y }
    }

    fn row(&self, y: u32, x0: u32, x1: u32) -> f32 {
        let base = (y * (self.w + 1)) as usize;
        let n = (self.sum_x[base + x1 as usize + 1] - self.sum_x[base + x0 as usize]) as f32;
        n / (x1 - x0 + 1) as f32
    }

    fn col(&self, x: u32, y0: u32, y1: u32) -> f32 {
        let n = (self.sum_y[((y1 + 1) * self.w + x) as usize] - self.sum_y[(y0 * self.w + x) as usize]) as f32;
        n / (y1 - y0 + 1) as f32
    }

    fn band_row(&self, y: u32, x0: u32, x1: u32) -> f32 {
        let a = y.saturating_sub(1);
        let b = (y + 1).min(self.h - 1);
        (a..=b).map(|yy| self.row(yy, x0, x1)).fold(0.0, f32::max)
    }

    fn band_col(&self, x: u32, y0: u32, y1: u32) -> f32 {
        let a = x.saturating_sub(1);
        let b = (x + 1).min(self.w - 1);
        (a..=b).map(|xx| self.col(xx, y0, y1)).fold(0.0, f32::max)
    }

    pub fn edge(&self, x: u32, y: u32) -> bool {
        self.on[(y * self.w + x) as usize]
    }
}

const SOLID: f32 = 0.72;
const MIN: u32 = 6;

impl Edges {
    fn side(&self, b: [u32; 4], k: usize) -> f32 {
        let [l, t, r, bt] = b;
        match k {
            0 => self.band_col(l, t, bt),
            1 => self.band_row(t, l, r),
            2 => self.band_col(r, t, bt),
            _ => self.band_row(bt, l, r),
        }
    }

    fn holds(&self, b: [u32; 4], k: usize) -> bool {
        let edge = match k {
            0 => b[0] == 0,
            1 => b[1] == 0,
            2 => b[2] + 1 >= self.w,
            _ => b[3] + 1 >= self.h,
        };
        edge || self.side(b, k) >= SOLID
    }

    fn release(&self, b: [u32; 4], k: usize) -> Option<[u32; 4]> {
        let mut c = b;
        let step = |c: &mut [u32; 4]| -> bool {
            match k {
                0 if c[0] > 0 => c[0] -= 1,
                1 if c[1] > 0 => c[1] -= 1,
                2 if c[2] + 1 < self.w => c[2] += 1,
                3 if c[3] + 1 < self.h => c[3] += 1,
                _ => return false,
            }
            true
        };
        for _ in 0..3 {
            if !step(&mut c) {
                return None;
            }
        }
        loop {
            if self.side(c, k) >= SOLID {
                break;
            }
            if !step(&mut c) {
                return None;
            }
        }
        (0..4).filter(|&o| o != k).all(|o| self.holds(c, o)).then_some(c)
    }
}

fn grow(e: &Edges, from: [u32; 4]) -> [u32; 4] {
    let (w, h) = (e.w, e.h);
    let [mut l, mut t, mut r, mut b] = from;
    let mut stop = [false; 4];
    while !stop.iter().all(|s| *s) {
        if !stop[0] {
            if l == 0 { stop[0] = true } else { l -= 1 }
        }
        if !stop[1] {
            if t == 0 { stop[1] = true } else { t -= 1 }
        }
        if !stop[2] {
            if r + 1 >= w { stop[2] = true } else { r += 1 }
        }
        if !stop[3] {
            if b + 1 >= h { stop[3] = true } else { b += 1 }
        }
        if r - l >= MIN && b - t >= MIN {
            for (k, done) in stop.iter_mut().enumerate() {
                if !*done && e.side([l, t, r, b], k) >= SOLID {
                    *done = true;
                }
            }
        }
    }
    [l, t, r, b]
}

pub fn boxes(img: &Image, at: (u32, u32)) -> Vec<[u32; 4]> {
    if img.w < 16 || img.h < 16 {
        return Vec::new();
    }
    let e = Edges::new(img);
    let (w, h) = (img.w, img.h);
    let (px, py) = (at.0.min(w - 1), at.1.min(h - 1));
    let area = |b: [u32; 4]| ((b[2] - b[0] + 1) * (b[3] - b[1] + 1)) as f32;
    let too_big = |b: [u32; 4]| area(b) > (w * h) as f32 * 0.85;
    let mut cur = grow(&e, [px, py, px, py]);
    let mut out: Vec<[u32; 4]> = Vec::new();
    for _ in 0..12 {
        if too_big(cur) {
            break;
        }
        if cur[2] - cur[0] + 1 >= MIN && cur[3] - cur[1] + 1 >= MIN && out.last().is_none_or(|p| area(cur) >= area(*p) * 1.2) {
            out.push(cur);
            if out.len() >= 6 {
                break;
            }
        }
        let next = (0..4).filter_map(|k| e.release(cur, k)).filter(|c| !too_big(*c)).min_by(|a, b| area(*a).total_cmp(&area(*b)));
        cur = match next {
            Some(n) => n,
            None => grow(&e, [cur[0].saturating_sub(3), cur[1].saturating_sub(3), (cur[2] + 3).min(w - 1), (cur[3] + 3).min(h - 1)]),
        };
    }
    out.into_iter().map(|b| [b[0], b[1], (b[2] - b[0] + 2).min(w - b[0]), (b[3] - b[1] + 2).min(h - b[1])]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(img: &mut Image, r: [u32; 4], c: [u8; 3]) {
        for y in r[1]..r[1] + r[3] {
            for x in r[0]..r[0] + r[2] {
                let i = ((y * img.w + x) * 4) as usize;
                img.rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }

    fn page() -> Image {
        let mut img = Image::new(400, 300);
        fill(&mut img, [0, 0, 400, 300], [245, 245, 247]);
        fill(&mut img, [60, 50, 260, 180], [255, 255, 255]);
        fill(&mut img, [100, 150, 90, 32], [59, 111, 224]);
        fill(&mut img, [110, 162, 40, 8], [255, 255, 255]);
        img
    }

    fn near(a: [u32; 4], b: [u32; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (*x as i64 - y as i64).abs() <= 1)
    }

    #[test]
    fn a_click_on_a_button_finds_the_button_then_its_card() {
        let found = boxes(&page(), (170, 158));
        assert!(found.len() >= 2, "{found:?}");
        assert!(near(found[0], [100, 150, 90, 32]), "button {found:?}");
        assert!(found.iter().any(|b| near(*b, [60, 50, 260, 180])), "card {found:?}");
    }

    #[test]
    fn a_click_on_plain_card_space_finds_the_card() {
        let found = boxes(&page(), (280, 80));
        assert!(near(found[0], [60, 50, 260, 180]), "{found:?}");
    }

    #[test]
    fn boxes_grow_and_never_cover_the_whole_image() {
        let img = page();
        for at in [(5, 5), (170, 158), (399, 299)] {
            let found = boxes(&img, at);
            for pair in found.windows(2) {
                assert!(pair[1][2] * pair[1][3] > pair[0][2] * pair[0][3]);
            }
            assert!(found.iter().all(|b| *b != [0, 0, 400, 300]));
        }
    }
}
