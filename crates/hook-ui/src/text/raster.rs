use super::fonts::Seg;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub right: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl Bounds {
    pub fn pixels(self) -> Bounds {
        Bounds {
            left: -((-self.left).floor()),
            right: self.right.ceil(),
            ascent: self.ascent.ceil(),
            descent: self.descent.ceil(),
        }
    }
}

pub fn cbox(segs: &[Seg]) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;
    let mut add = |x: f32, y: f32| {
        let c = b.get_or_insert(Bounds {
            left: -x,
            right: x,
            ascent: y,
            descent: -y,
        });
        c.left = c.left.max(-x);
        c.right = c.right.max(x);
        c.ascent = c.ascent.max(y);
        c.descent = c.descent.max(-y);
    };
    for s in segs {
        match *s {
            Seg::Move(x, y) | Seg::Line(x, y) => add(x, y),
            Seg::Quad(a, b, x, y) => {
                add(a, b);
                add(x, y);
            }
            Seg::Cubic(a, b, c, d, x, y) => {
                add(a, b);
                add(c, d);
                add(x, y);
            }
            Seg::Close => {}
        }
    }
    b
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

pub fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn lerp(t: f32, a: Point, b: Point) -> Point {
    point(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y))
}

fn distance(a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    (dx * dx + dy * dy).sqrt()
}

pub struct Rasterizer {
    width: usize,
    height: usize,
    a: Vec<f32>,
}

impl Rasterizer {
    pub fn new(width: usize, height: usize) -> Self {
        Rasterizer {
            width,
            height,
            a: vec![0.0; width * height + 4],
        }
    }

    fn add(&mut self, i: usize, v: f32) -> bool {
        match self.a.get_mut(i) {
            Some(c) => {
                *c += v;
                true
            }
            None => false,
        }
    }

    pub fn draw_line(&mut self, p0: Point, p1: Point) {
        if (p0.y - p1.y).abs() <= f32::EPSILON {
            return;
        }
        let (dir, p0, p1) = if p0.y < p1.y { (1.0, p0, p1) } else { (-1.0, p1, p0) };
        let dxdy = (p1.x - p0.x) / (p1.y - p0.y);
        let mut x = p0.x;
        let y0 = p0.y as usize;
        if p0.y < 0.0 {
            x -= p0.y * dxdy;
        }
        for y in y0..self.height.min(p1.y.ceil() as usize) {
            let line = y * self.width;
            let dy = ((y + 1) as f32).min(p1.y) - (y as f32).max(p0.y);
            let xnext = x + dxdy * dy;
            let d = dy * dir;
            let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let x0floor = x0.floor();
            let x0i = x0floor as i32;
            let x1ceil = x1.ceil();
            let x1i = x1ceil as i32;
            let start = line as isize + x0i as isize;
            if start < 0 {
                continue;
            }
            let start = start as usize;
            if x1i <= x0i + 1 {
                let xmf = 0.5 * (x + xnext) - x0floor;
                if !self.add(start, d - d * xmf) || !self.add(start + 1, d * xmf) {
                    continue;
                }
            } else {
                let s = (x1 - x0).recip();
                let x0f = x0 - x0floor;
                let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
                let x1f = x1 - x1ceil + 1.0;
                let am = 0.5 * s * x1f * x1f;
                if !self.add(start, d * a0) {
                    continue;
                }
                if x1i == x0i + 2 {
                    if !self.add(start + 1, d * (1.0 - a0 - am)) {
                        continue;
                    }
                } else {
                    let a1 = s * (1.5 - x0f);
                    if !self.add(start + 1, d * (a1 - a0)) {
                        continue;
                    }
                    for xi in x0i + 2..x1i - 1 {
                        self.add(line + xi as usize, d * s);
                    }
                    let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                    if !self.add(line + (x1i - 1) as usize, d * (1.0 - a2 - am)) {
                        continue;
                    }
                }
                if !self.add(line + x1i as usize, d * am) {
                    continue;
                }
            }
            x = xnext;
        }
    }

    pub fn draw_quad(&mut self, p0: Point, p1: Point, p2: Point) {
        let devx = p0.x - 2.0 * p1.x + p2.x;
        let devy = p0.y - 2.0 * p1.y + p2.y;
        let devsq = devx * devx + devy * devy;
        if devsq < 0.333 {
            self.draw_line(p0, p2);
            return;
        }
        let n = 1 + (3.0 * devsq).sqrt().sqrt().floor() as usize;
        let mut p = p0;
        let step = (n as f32).recip();
        let mut t = 0.0;
        for _ in 0..n - 1 {
            t += step;
            let pn = lerp(t, lerp(t, p0, p1), lerp(t, p1, p2));
            self.draw_line(p, pn);
            p = pn;
        }
        self.draw_line(p, p2);
    }

    pub fn draw_cubic(&mut self, p0: Point, p1: Point, p2: Point, p3: Point) {
        self.cubic(p0, p1, p2, p3, 0);
    }

    fn cubic(&mut self, p0: Point, p1: Point, p2: Point, p3: Point, n: u8) {
        let long = distance(p0, p1) + distance(p1, p2) + distance(p2, p3);
        let short = distance(p0, p3);
        if n < 16 && long * long - short * short > 0.35 * 0.35 {
            let p01 = lerp(0.5, p0, p1);
            let p12 = lerp(0.5, p1, p2);
            let p23 = lerp(0.5, p2, p3);
            let pa = lerp(0.5, p01, p12);
            let pb = lerp(0.5, p12, p23);
            let mp = lerp(0.5, pa, pb);
            self.cubic(p0, p01, pa, mp, n + 1);
            self.cubic(mp, pb, p23, p3, n + 1);
        } else {
            self.draw_line(p0, p3);
        }
    }

    pub fn for_each_pixel(&self, mut f: impl FnMut(usize, f32)) {
        let mut acc = 0.0;
        for (i, c) in self.a[..self.width * self.height].iter().enumerate() {
            acc += c;
            f(i, acc.abs());
        }
    }
}

pub fn coverage(segs: &[Seg], ox: f32, oy: f32, w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0; w * h];
    if w == 0 || h == 0 || segs.is_empty() {
        return out;
    }
    let tf = |x: f32, y: f32| point(ox + x, oy - y);
    let mut r = Rasterizer::new(w, h);
    let mut start = point(0.0, 0.0);
    let mut cur = start;
    let mut open = false;
    for seg in segs {
        match *seg {
            Seg::Move(x, y) => {
                if open && cur != start {
                    r.draw_line(cur, start);
                }
                start = tf(x, y);
                cur = start;
                open = true;
            }
            Seg::Line(x, y) => {
                let p = tf(x, y);
                r.draw_line(cur, p);
                cur = p;
            }
            Seg::Quad(x1, y1, x, y) => {
                let p = tf(x, y);
                r.draw_quad(cur, tf(x1, y1), p);
                cur = p;
            }
            Seg::Cubic(x1, y1, x2, y2, x, y) => {
                let p = tf(x, y);
                r.draw_cubic(cur, tf(x1, y1), tf(x2, y2), p);
                cur = p;
            }
            Seg::Close => {
                if open && cur != start {
                    r.draw_line(cur, start);
                }
                cur = start;
                open = false;
            }
        }
    }
    if open && cur != start {
        r.draw_line(cur, start);
    }
    r.for_each_pixel(|i, v| out[i] = v.clamp(0.0, 1.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_aligned_square_fully_covers_its_pixels() {
        let mut r = Rasterizer::new(4, 4);
        let (a, b, c, d) = (point(1.0, 1.0), point(3.0, 1.0), point(3.0, 3.0), point(1.0, 3.0));
        r.draw_line(a, b);
        r.draw_line(b, c);
        r.draw_line(c, d);
        r.draw_line(d, a);
        let mut v = vec![0.0; 16];
        r.for_each_pixel(|i, c| v[i] = c);
        assert_eq!(v, [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn half_a_pixel_covers_half() {
        let mut r = Rasterizer::new(2, 1);
        let (a, b, c, d) = (point(0.5, 0.0), point(1.0, 0.0), point(1.0, 1.0), point(0.5, 1.0));
        r.draw_line(a, b);
        r.draw_line(b, c);
        r.draw_line(c, d);
        r.draw_line(d, a);
        let mut v = vec![0.0; 2];
        r.for_each_pixel(|i, c| v[i] = c);
        assert_eq!(v, [0.5, 0.0]);
    }
}
