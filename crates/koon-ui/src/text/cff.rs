use super::fonts::Seg;

const ONE: i32 = 0x10000;
const MIN_COUNTER: i32 = 0x8000;

pub fn mul_fix(a: i32, b: i32) -> i32 {
    let s = (a as i64 * b as i64).signum();
    let p = (a as i64).abs() * (b as i64).abs();
    (s * ((p + 0x8000) >> 16)) as i32
}

pub fn div_fix(a: i32, b: i32) -> i32 {
    if b == 0 {
        return 0x7fff_ffff;
    }
    let s = (a as i64).signum() * (b as i64).signum();
    let (a, b) = ((a as i64).abs(), (b as i64).abs());
    (s * (((a << 16) + (b >> 1)) / b)) as i32
}

fn round(x: i32) -> i32 {
    (x.wrapping_add(0x8000)) & !0xffff
}

fn frac(x: i32) -> i32 {
    x & 0xffff
}

struct Index<'a> {
    data: &'a [u8],
    offsets: Vec<usize>,
    end: usize,
}

fn read_index(data: &[u8], at: usize) -> Option<Index<'_>> {
    let count = u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]) as usize;
    if count == 0 {
        return Some(Index {
            data,
            offsets: vec![at + 2],
            end: at + 2,
        });
    }
    let size = *data.get(at + 2)? as usize;
    let base = at + 3 + (count + 1) * size - 1;
    let mut offsets = Vec::with_capacity(count + 1);
    for i in 0..=count {
        let mut v = 0usize;
        for k in 0..size {
            v = v << 8 | *data.get(at + 3 + i * size + k)? as usize;
        }
        offsets.push(base + v);
    }
    let end = *offsets.last()?;
    Some(Index { data, offsets, end })
}

impl<'a> Index<'a> {
    fn len(&self) -> usize {
        self.offsets.len() - 1
    }
    fn get(&self, i: usize) -> Option<&'a [u8]> {
        if i + 1 >= self.offsets.len() {
            return None;
        }
        self.data.get(self.offsets[i]..self.offsets[i + 1])
    }
}

fn dict(data: &[u8]) -> Vec<(u16, Vec<f64>)> {
    let mut out = Vec::new();
    let mut args = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            0..=21 => {
                let op = if b == 12 {
                    i += 1;
                    1200 + *data.get(i).unwrap_or(&0) as u16
                } else {
                    b as u16
                };
                out.push((op, std::mem::take(&mut args)));
                i += 1;
            }
            28 if i + 2 < data.len() => {
                args.push(i16::from_be_bytes([data[i + 1], data[i + 2]]) as f64);
                i += 3;
            }
            29 if i + 4 < data.len() => {
                args.push(i32::from_be_bytes([data[i + 1], data[i + 2], data[i + 3], data[i + 4]]) as f64);
                i += 5;
            }
            30 => {
                let mut s = String::new();
                i += 1;
                'real: while i < data.len() {
                    for nib in [data[i] >> 4, data[i] & 15] {
                        match nib {
                            0..=9 => s.push((b'0' + nib) as char),
                            10 => s.push('.'),
                            11 => s.push('E'),
                            12 => s.push_str("E-"),
                            14 => s.push('-'),
                            15 => {
                                i += 1;
                                break 'real;
                            }
                            _ => {}
                        }
                    }
                    i += 1;
                }
                args.push(s.parse().unwrap_or(0.0));
            }
            32..=246 => {
                args.push(b as f64 - 139.0);
                i += 1;
            }
            247..=250 if i + 1 < data.len() => {
                args.push(((b as f64 - 247.0) * 256.0) + data[i + 1] as f64 + 108.0);
                i += 2;
            }
            251..=254 if i + 1 < data.len() => {
                args.push(-((b as f64 - 251.0) * 256.0) - data[i + 1] as f64 - 108.0);
                i += 2;
            }
            _ => i += 1,
        }
    }
    out
}

#[derive(Clone, Copy, Debug)]
struct Zone {
    bottom: i32,
    top: i32,
    flat: i32,
    bottom_zone: bool,
    ds_flat: i32,
}

pub struct Cff<'a> {
    charstrings: Index<'a>,
    gsubrs: Index<'a>,
    lsubrs: Option<Index<'a>>,
    blue_values: Vec<i32>,
    other_blues: Vec<i32>,
    blue_scale: i32,
    blue_shift: i32,
    blue_fuzz: i32,
}

fn bias(n: usize) -> i32 {
    if n < 1240 {
        107
    } else if n < 33900 {
        1131
    } else {
        32768
    }
}

impl<'a> Cff<'a> {
    pub fn parse(data: &'a [u8]) -> Option<Cff<'a>> {
        let hdr = *data.get(2)? as usize;
        let names = read_index(data, hdr)?;
        let tops = read_index(data, names.end)?;
        let strings = read_index(data, tops.end)?;
        let gsubrs = read_index(data, strings.end)?;
        let top = dict(tops.get(0)?);
        let find = |d: &[(u16, Vec<f64>)], op: u16| d.iter().find(|(o, _)| *o == op).map(|(_, a)| a.clone());
        let cs_at = find(&top, 17)?.first().copied()? as usize;
        let charstrings = read_index(data, cs_at)?;
        let (mut blue_values, mut other_blues) = (Vec::new(), Vec::new());
        let (mut blue_scale, mut blue_shift, mut blue_fuzz) = (0.039625, 7.0, 1.0);
        let mut lsubrs = None;
        if let Some(p) = find(&top, 18)
            && p.len() == 2
        {
            let (size, at) = (p[0] as usize, p[1] as usize);
            let private = dict(data.get(at..at + size)?);
            let deltas = |v: Vec<f64>| {
                let mut acc = 0.0;
                v.into_iter()
                    .map(|d| {
                        acc += d;
                        (acc * 65536.0).round() as i32
                    })
                    .collect::<Vec<i32>>()
            };
            if let Some(v) = find(&private, 6) {
                blue_values = deltas(v);
            }
            if let Some(v) = find(&private, 7) {
                other_blues = deltas(v);
            }
            if let Some(v) = find(&private, 1209) {
                blue_scale = v.first().copied().unwrap_or(blue_scale);
            }
            if let Some(v) = find(&private, 1210) {
                blue_shift = v.first().copied().unwrap_or(blue_shift);
            }
            if let Some(v) = find(&private, 1211) {
                blue_fuzz = v.first().copied().unwrap_or(blue_fuzz);
            }
            if let Some(v) = find(&private, 19) {
                lsubrs = read_index(data, at + v.first().copied().unwrap_or(0.0) as usize);
            }
        }
        let scaled = (blue_scale * 65536.0 * 1000.0) as i64;
        Some(Cff {
            charstrings,
            gsubrs,
            lsubrs,
            blue_values,
            other_blues,
            blue_scale: div_fix(scaled.min(i32::MAX as i64) as i32, 1000 << 16),
            blue_shift: (blue_shift as i32) << 16,
            blue_fuzz: (blue_fuzz as i32) << 16,
        })
    }

    fn charstring(&self, gid: u16) -> Option<Program> {
        let mut p = Program::default();
        let mut st = State::default();
        self.run(self.charstrings.get(gid as usize)?, &mut p, &mut st, 0)?;
        if st.open {
            p.ops.push(Op::Close);
        }
        Some(p)
    }

    fn run(&self, code: &[u8], p: &mut Program, st: &mut State, depth: u32) -> Option<bool> {
        if depth > 10 {
            return None;
        }
        let mut i = 0;
        while i < code.len() {
            let b = code[i];
            i += 1;
            match b {
                28 => {
                    st.stack.push((i16::from_be_bytes([*code.get(i)?, *code.get(i + 1)?]) as i32) << 16);
                    i += 2;
                }
                32..=246 => st.stack.push((b as i32 - 139) << 16),
                247..=250 => {
                    st.stack.push(((b as i32 - 247) * 256 + *code.get(i)? as i32 + 108) << 16);
                    i += 1;
                }
                251..=254 => {
                    st.stack.push((-(b as i32 - 251) * 256 - *code.get(i)? as i32 - 108) << 16);
                    i += 1;
                }
                255 => {
                    st.stack.push(i32::from_be_bytes([*code.get(i)?, *code.get(i + 1)?, *code.get(i + 2)?, *code.get(i + 3)?]));
                    i += 4;
                }
                1 | 18 => {
                    st.width(st.stack.len() % 2 == 1);
                    let mut y = 0;
                    for pair in st.stack.chunks(2) {
                        if pair.len() == 2 {
                            let min = y + pair[0];
                            let max = min + pair[1];
                            p.hstems.push((min, max));
                            y = max;
                        }
                    }
                    st.stack.clear();
                }
                3 | 23 => {
                    st.width(st.stack.len() % 2 == 1);
                    p.vstems += st.stack.len() / 2;
                    st.stack.clear();
                }
                19 | 20 => {
                    st.width(st.stack.len() % 2 == 1);
                    p.vstems += st.stack.len() / 2;
                    st.stack.clear();
                    let n = (p.hstems.len() + p.vstems).div_ceil(8);
                    let bytes = code.get(i..i + n)?;
                    i += n;
                    if b == 19 {
                        let mask = (0..p.hstems.len()).map(|k| bytes[k / 8] & (0x80 >> (k % 8)) != 0).collect();
                        p.ops.push(Op::Mask(mask));
                    }
                }
                21 => {
                    st.width(st.stack.len() > 2);
                    let n = st.stack.len();
                    let (dx, dy) = (*st.stack.get(n.wrapping_sub(2))?, *st.stack.get(n.wrapping_sub(1))?);
                    st.move_to(p, dx, dy);
                }
                22 => {
                    st.width(st.stack.len() > 1);
                    let dx = *st.stack.last()?;
                    st.move_to(p, dx, 0);
                }
                4 => {
                    st.width(st.stack.len() > 1);
                    let dy = *st.stack.last()?;
                    st.move_to(p, 0, dy);
                }
                5 => {
                    let s = std::mem::take(&mut st.stack);
                    for d in s.chunks(2) {
                        if d.len() == 2 {
                            st.line(p, d[0], d[1]);
                        }
                    }
                }
                6 | 7 => {
                    let s = std::mem::take(&mut st.stack);
                    let mut horizontal = b == 6;
                    for &d in &s {
                        if horizontal {
                            st.line(p, d, 0);
                        } else {
                            st.line(p, 0, d);
                        }
                        horizontal = !horizontal;
                    }
                }
                8 => {
                    let s = std::mem::take(&mut st.stack);
                    for c in s.chunks(6) {
                        if c.len() == 6 {
                            st.curve(p, c[0], c[1], c[2], c[3], c[4], c[5]);
                        }
                    }
                }
                24 => {
                    let s = std::mem::take(&mut st.stack);
                    if s.len() < 8 {
                        return None;
                    }
                    let n = s.len() - 2;
                    for c in s[..n].chunks(6) {
                        if c.len() == 6 {
                            st.curve(p, c[0], c[1], c[2], c[3], c[4], c[5]);
                        }
                    }
                    st.line(p, s[n], s[n + 1]);
                }
                25 => {
                    let s = std::mem::take(&mut st.stack);
                    if s.len() < 8 {
                        return None;
                    }
                    let n = s.len() - 6;
                    for d in s[..n].chunks(2) {
                        if d.len() == 2 {
                            st.line(p, d[0], d[1]);
                        }
                    }
                    st.curve(p, s[n], s[n + 1], s[n + 2], s[n + 3], s[n + 4], s[n + 5]);
                }
                26 => {
                    let s = std::mem::take(&mut st.stack);
                    let mut k = 0;
                    let mut dx1 = 0;
                    if s.len() % 4 == 1 {
                        dx1 = s[0];
                        k = 1;
                    }
                    while k + 4 <= s.len() {
                        st.curve(p, dx1, s[k], s[k + 1], s[k + 2], 0, s[k + 3]);
                        dx1 = 0;
                        k += 4;
                    }
                }
                27 => {
                    let s = std::mem::take(&mut st.stack);
                    let mut k = 0;
                    let mut dy1 = 0;
                    if s.len() % 4 == 1 {
                        dy1 = s[0];
                        k = 1;
                    }
                    while k + 4 <= s.len() {
                        st.curve(p, s[k], dy1, s[k + 1], s[k + 2], s[k + 3], 0);
                        dy1 = 0;
                        k += 4;
                    }
                }
                30 | 31 => {
                    let s = std::mem::take(&mut st.stack);
                    let mut horizontal = b == 31;
                    let mut k = 0;
                    while k + 4 <= s.len() {
                        let last = k + 4 == s.len() - 1;
                        let extra = if last { s[k + 4] } else { 0 };
                        if horizontal {
                            st.curve(p, s[k], 0, s[k + 1], s[k + 2], extra, s[k + 3]);
                        } else {
                            st.curve(p, 0, s[k], s[k + 1], s[k + 2], s[k + 3], extra);
                        }
                        horizontal = !horizontal;
                        k += if last { 5 } else { 4 };
                    }
                }
                10 => {
                    let n = st.stack.pop()? >> 16;
                    let subrs = self.lsubrs.as_ref()?;
                    let code = subrs.get((n + bias(subrs.len())) as usize)?;
                    if self.run(code, p, st, depth + 1)? {
                        return Some(true);
                    }
                }
                29 => {
                    let n = st.stack.pop()? >> 16;
                    let code = self.gsubrs.get((n + bias(self.gsubrs.len())) as usize)?;
                    if self.run(code, p, st, depth + 1)? {
                        return Some(true);
                    }
                }
                11 => return Some(false),
                14 => {
                    if st.open {
                        p.ops.push(Op::Close);
                        st.open = false;
                    }
                    st.stack.clear();
                    return Some(true);
                }
                12 => {
                    let e = *code.get(i)?;
                    i += 1;
                    let s = std::mem::take(&mut st.stack);
                    match e {
                        35 if s.len() >= 12 => {
                            st.curve(p, s[0], s[1], s[2], s[3], s[4], s[5]);
                            st.curve(p, s[6], s[7], s[8], s[9], s[10], s[11]);
                        }
                        34 if s.len() >= 7 => {
                            let y0 = st.y;
                            st.curve(p, s[0], 0, s[1], s[2], s[3], 0);
                            st.curve(p, s[4], 0, s[5], y0 - st.y, s[6], 0);
                        }
                        36 if s.len() >= 9 => {
                            let y0 = st.y;
                            st.curve(p, s[0], s[1], s[2], s[3], s[4], 0);
                            let dy = y0 - (st.y + s[7]);
                            st.curve(p, s[5], 0, s[6], s[7], s[8], dy);
                        }
                        37 if s.len() >= 11 => {
                            let (x0, y0) = (st.x, st.y);
                            let dx = s[0] + s[2] + s[4] + s[6] + s[8];
                            let dy = s[1] + s[3] + s[5] + s[7] + s[9];
                            st.curve(p, s[0], s[1], s[2], s[3], s[4], s[5]);
                            let (ex, ey) = if dx.abs() > dy.abs() {
                                (s[10], y0 - (st.y + s[7] + s[9]))
                            } else {
                                (x0 - (st.x + s[6] + s[8]), s[10])
                            };
                            st.curve(p, s[6], s[7], s[8], s[9], ex, ey);
                        }
                        _ => {}
                    }
                }
                _ => st.stack.clear(),
            }
        }
        Some(false)
    }
}

#[derive(Default)]
struct State {
    stack: Vec<i32>,
    x: i32,
    y: i32,
    open: bool,
    has_width: bool,
}

impl State {
    fn width(&mut self, extra: bool) {
        if !self.has_width {
            self.has_width = true;
            if extra && !self.stack.is_empty() {
                self.stack.remove(0);
            }
        }
    }
    fn move_to(&mut self, p: &mut Program, dx: i32, dy: i32) {
        if self.open {
            p.ops.push(Op::Close);
        }
        self.x += dx;
        self.y += dy;
        p.ops.push(Op::Move(self.x, self.y));
        self.open = true;
        self.stack.clear();
    }
    fn line(&mut self, p: &mut Program, dx: i32, dy: i32) {
        self.x += dx;
        self.y += dy;
        p.ops.push(Op::Line(self.x, self.y));
    }
    #[allow(clippy::too_many_arguments)]
    fn curve(&mut self, p: &mut Program, a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {
        let x1 = self.x + a;
        let y1 = self.y + b;
        let x2 = x1 + c;
        let y2 = y1 + d;
        self.x = x2 + e;
        self.y = y2 + f;
        p.ops.push(Op::Curve(x1, y1, x2, y2, self.x, self.y));
    }
}

#[derive(Clone, Debug)]
enum Op {
    Mask(Vec<bool>),
    Move(i32, i32),
    Line(i32, i32),
    Curve(i32, i32, i32, i32, i32, i32),
    Close,
}

#[derive(Default)]
struct Program {
    hstems: Vec<(i32, i32)>,
    vstems: usize,
    ops: Vec<Op>,
}

const PAIR_BOTTOM: u8 = 1;
const PAIR_TOP: u8 = 2;
const GHOST_BOTTOM: u8 = 4;
const GHOST_TOP: u8 = 8;
const LOCKED: u8 = 16;
const SYNTHETIC: u8 = 32;

#[derive(Clone, Copy, Default, Debug)]
struct Hint {
    cs: i32,
    ds: i32,
    scale: i32,
    flags: u8,
    index: usize,
}

impl Hint {
    fn valid(&self) -> bool {
        self.flags != 0
    }
    fn is_pair(&self) -> bool {
        self.flags & (PAIR_BOTTOM | PAIR_TOP) != 0
    }
    fn is_top(&self) -> bool {
        self.flags & (PAIR_TOP | GHOST_TOP) != 0
    }
    fn is_bottom(&self) -> bool {
        self.flags & (PAIR_BOTTOM | GHOST_BOTTOM) != 0
    }
    fn locked(&self) -> bool {
        self.flags & LOCKED != 0
    }
}

#[derive(Clone, Copy, Default)]
struct Stem {
    min: i32,
    max: i32,
    used: bool,
    min_ds: i32,
    max_ds: i32,
}

#[derive(Clone, Default)]
struct HintMap {
    edges: Vec<Hint>,
    scale: i32,
    valid: bool,
}

impl HintMap {
    fn map(&self, cs: i32) -> i32 {
        if self.edges.is_empty() {
            return mul_fix(cs, self.scale);
        }
        let mut i = 0;
        while i + 1 < self.edges.len() && cs >= self.edges[i + 1].cs {
            i += 1;
        }
        if i == 0 && cs < self.edges[0].cs {
            return mul_fix(cs - self.edges[0].cs, self.scale) + self.edges[0].ds;
        }
        mul_fix(cs - self.edges[i].cs, self.edges[i].scale) + self.edges[i].ds
    }

    fn insert(&mut self, initial: Option<&HintMap>, bottom: Hint, top: Hint) {
        let mut pair = true;
        let (mut first, mut second) = (bottom, top);
        if !bottom.valid() {
            first = top;
            pair = false;
        } else if !top.valid() {
            pair = false;
        }
        if pair && top.cs < bottom.cs {
            return;
        }
        let at = self.edges.iter().position(|e| e.cs >= first.cs).unwrap_or(self.edges.len());
        if at < self.edges.len() {
            if self.edges[at].cs == first.cs {
                return;
            }
            if pair && self.edges[at].cs <= second.cs {
                return;
            }
            if self.edges[at].flags & PAIR_TOP != 0 {
                return;
            }
        }
        if let Some(init) = initial
            && init.valid
            && !first.locked()
        {
            if pair {
                let mid = init.map(first.cs + (second.cs - first.cs) / 2);
                let half = mul_fix((second.cs - first.cs) / 2, self.scale);
                first.ds = mid - half;
                second.ds = mid + half;
            } else {
                first.ds = init.map(first.cs);
            }
        }
        if at > 0 && first.ds < self.edges[at - 1].ds {
            return;
        }
        if at < self.edges.len() {
            let probe = if pair { second.ds } else { first.ds };
            if probe > self.edges[at].ds {
                return;
            }
        }
        if pair {
            self.edges.insert(at, second);
        }
        self.edges.insert(at, first);
    }

    fn adjust(&mut self) {
        let mut moves: Vec<(usize, i32)> = Vec::new();
        let n = self.edges.len();
        let mut i = 0;
        while i < n {
            let pair = self.edges[i].is_pair();
            let j = if pair { i + 1 } else { i };
            if j >= n {
                break;
            }
            let di = self.edges[i].ds;
            let dj = self.edges[j].ds;
            if !self.edges[i].locked() {
                let fd = frac(di);
                let fu = frac(dj);
                let down_down = -fd;
                let up_down = -fu;
                let down_up = if fd == 0 { 0 } else { ONE - fd };
                let up_up = if fu == 0 { 0 } else { ONE - fu };
                let move_up = down_up.min(up_up);
                let move_down = down_down.max(up_down);
                let mut save = false;
                let mv;
                if j >= n - 1 || self.edges[j + 1].ds >= dj + move_up + MIN_COUNTER {
                    if i == 0 || self.edges[i - 1].ds <= di + move_down - MIN_COUNTER {
                        mv = if -move_down < move_up { move_down } else { move_up };
                    } else {
                        mv = move_up;
                    }
                } else if i == 0 || self.edges[i - 1].ds <= di + move_down - MIN_COUNTER {
                    mv = move_down;
                    save = move_up < -move_down;
                } else {
                    mv = 0;
                    save = true;
                }
                if save && j < n - 1 && !self.edges[j + 1].locked() {
                    moves.push((j, move_up - mv));
                }
                self.edges[i].ds = di + mv;
                if pair {
                    self.edges[j].ds = dj + mv;
                }
            }
            if i > 0 && self.edges[i].cs != self.edges[i - 1].cs {
                self.edges[i - 1].scale = div_fix(self.edges[i].ds - self.edges[i - 1].ds, self.edges[i].cs - self.edges[i - 1].cs);
            }
            if pair {
                if self.edges[j].cs != self.edges[j - 1].cs {
                    self.edges[j - 1].scale = div_fix(self.edges[j].ds - self.edges[j - 1].ds, self.edges[j].cs - self.edges[j - 1].cs);
                }
                i += 1;
            }
            i += 1;
        }
        for &(j, up) in moves.iter().rev() {
            if self.edges[j + 1].ds >= self.edges[j].ds + up + MIN_COUNTER {
                self.edges[j].ds += up;
                if self.edges[j].is_pair() && j > 0 {
                    self.edges[j - 1].ds += up;
                }
            }
        }
    }
}

struct Hinter<'a> {
    zones: Vec<Zone>,
    suppress: bool,
    shift: i32,
    fuzz: i32,
    scale: i32,
    stems: &'a mut Vec<Stem>,
}

impl Hinter<'_> {
    fn edge(&self, i: usize, bottom: bool) -> Hint {
        let s = self.stems[i];
        let width = s.max - s.min;
        let mut h = Hint::default();
        if width == -21 << 16 {
            if bottom {
                h.cs = s.max;
                h.flags = GHOST_BOTTOM;
            }
        } else if width == -20 << 16 {
            if !bottom {
                h.cs = s.min;
                h.flags = GHOST_TOP;
            }
        } else if width < 0 {
            if bottom {
                h.cs = s.max;
                h.flags = PAIR_BOTTOM;
            } else {
                h.cs = s.min;
                h.flags = PAIR_TOP;
            }
        } else if bottom {
            h.cs = s.min;
            h.flags = PAIR_BOTTOM;
        } else {
            h.cs = s.max;
            h.flags = PAIR_TOP;
        }
        h.scale = self.scale;
        h.index = i;
        if h.flags != 0 && s.used {
            h.ds = if h.is_top() { s.max_ds } else { s.min_ds };
            h.flags |= LOCKED;
        } else {
            h.ds = mul_fix(h.cs, self.scale);
        }
        h
    }

    fn capture(&self, bottom: &mut Hint, top: &mut Hint) -> bool {
        let mut mv = 0;
        let mut captured = false;
        for z in &self.zones {
            if z.bottom_zone && bottom.is_bottom() {
                if z.bottom - self.fuzz <= bottom.cs && bottom.cs <= z.top + self.fuzz {
                    let new = if self.suppress {
                        z.ds_flat
                    } else if z.top - bottom.cs > self.shift {
                        round(bottom.ds).min(z.ds_flat - ONE)
                    } else {
                        round(bottom.ds)
                    };
                    mv = new - bottom.ds;
                    captured = true;
                    break;
                }
            }
            if !z.bottom_zone && top.is_top() && z.bottom - self.fuzz <= top.cs && top.cs <= z.top + self.fuzz {
                let new = if self.suppress {
                    z.ds_flat
                } else if top.cs - z.bottom > self.shift {
                    round(top.ds).max(z.ds_flat + ONE)
                } else {
                    round(top.ds)
                };
                mv = new - top.ds;
                captured = true;
                break;
            }
        }
        if captured {
            if bottom.valid() {
                bottom.ds += mv;
                bottom.flags |= LOCKED;
            }
            if top.valid() {
                top.ds += mv;
                top.flags |= LOCKED;
            }
        }
        captured
    }

    fn build(&mut self, initial: Option<&HintMap>, mask: &[bool], is_initial: bool) -> HintMap {
        let mut map = HintMap {
            edges: Vec::new(),
            scale: self.scale,
            valid: false,
        };
        let mut rest = mask.to_vec();
        for (i, on) in rest.iter_mut().enumerate() {
            if !*on {
                continue;
            }
            let mut b = self.edge(i, true);
            let mut t = self.edge(i, false);
            if b.locked() || t.locked() || self.capture(&mut b, &mut t) {
                map.insert(initial, b, t);
                *on = false;
            }
        }
        if is_initial {
            if map.edges.is_empty() || map.edges[0].cs > 0 || map.edges[map.edges.len() - 1].cs < 0 {
                let edge = Hint {
                    flags: GHOST_BOTTOM | LOCKED | SYNTHETIC,
                    scale: self.scale,
                    ..Default::default()
                };
                map.insert(initial, edge, Hint::default());
            }
        } else {
            for (i, &on) in rest.iter().enumerate() {
                if on {
                    let b = self.edge(i, true);
                    let t = self.edge(i, false);
                    map.insert(initial, b, t);
                }
            }
        }
        map.adjust();
        if !is_initial {
            for e in &map.edges {
                if e.flags & SYNTHETIC == 0 {
                    let s = &mut self.stems[e.index];
                    if e.is_top() {
                        s.max_ds = e.ds;
                    } else {
                        s.min_ds = e.ds;
                    }
                    s.used = true;
                }
            }
        }
        map.valid = true;
        map
    }
}

impl Cff<'_> {
    pub fn path(&self, gid: u16, x_scale: i32, y_scale: i32, hinted: bool) -> Option<Vec<Seg>> {
        let p = self.charstring(gid)?;
        let mut stems: Vec<Stem> = p.hstems.iter().map(|&(min, max)| Stem { min, max, ..Default::default() }).collect();
        let mut zones = Vec::new();
        for (k, pair) in self.blue_values.chunks(2).enumerate() {
            if pair.len() == 2 && pair[1] >= pair[0] {
                let bottom_zone = k == 0;
                zones.push(Zone {
                    bottom: pair[0],
                    top: pair[1],
                    flat: if bottom_zone { pair[1] } else { pair[0] },
                    bottom_zone,
                    ds_flat: 0,
                });
            }
        }
        for pair in self.other_blues.chunks(2) {
            if pair.len() == 2 && pair[1] >= pair[0] {
                zones.push(Zone {
                    bottom: pair[0],
                    top: pair[1],
                    flat: pair[1],
                    bottom_zone: true,
                    ds_flat: 0,
                });
            }
        }
        let max_zone = zones.iter().map(|z| z.top - z.bottom).max().unwrap_or(0);
        let mut blue_scale = self.blue_scale;
        if max_zone > 0 && blue_scale > div_fix(ONE, max_zone) {
            blue_scale = div_fix(ONE, max_zone);
        }
        let suppress = y_scale < blue_scale;
        let boost = if suppress {
            let b = blue_scale.max(1) as i64;
            (0x9999 - ((0x9999i64 * y_scale as i64 + b / 2) / b) as i32).min(0x7fff)
        } else {
            0
        };
        for z in &mut zones {
            z.ds_flat = if z.bottom_zone {
                round(mul_fix(z.flat, y_scale) - boost)
            } else {
                round(mul_fix(z.flat, y_scale) + boost)
            };
        }
        let mut h = Hinter {
            zones,
            suppress,
            shift: self.blue_shift,
            fuzz: self.blue_fuzz,
            scale: y_scale,
            stems: &mut stems,
        };
        let all = vec![true; p.hstems.len()];
        let initial = if hinted { Some(h.build(None, &all, true)) } else { None };
        let mut mask = all.clone();
        let mut fresh = true;
        let mut map = HintMap {
            edges: Vec::new(),
            scale: y_scale,
            valid: false,
        };
        let mut first = map.clone();
        let fx = |v: i32| (mul_fix(v, x_scale) >> 10) as f32 / 64.0;
        let fy = |m: &HintMap, v: i32| ((if hinted { m.map(v) } else { mul_fix(v, y_scale) }) >> 10) as f32 / 64.0;
        let mut out = Vec::new();
        let mut start = (0, 0);
        let mut last: Option<(i32, i32, usize)> = None;
        for op in &p.ops {
            match op {
                Op::Mask(m) => {
                    mask = m.clone();
                    fresh = true;
                }
                Op::Move(x, y) => {
                    if hinted && (!map.valid || fresh) {
                        map = h.build(initial.as_ref(), &mask, false);
                        fresh = false;
                    }
                    first = map.clone();
                    start = (*x, *y);
                    out.push(Seg::Move(fx(*x), fy(&first, *y)));
                    last = None;
                }
                Op::Line(x, y) => {
                    if hinted && fresh {
                        map = h.build(initial.as_ref(), &mask, false);
                        fresh = false;
                    }
                    out.push(Seg::Line(fx(*x), fy(&map, *y)));
                    last = Some((*x, *y, out.len() - 1));
                }
                Op::Curve(x1, y1, x2, y2, x, y) => {
                    if hinted && fresh {
                        map = h.build(initial.as_ref(), &mask, false);
                        fresh = false;
                    }
                    out.push(Seg::Cubic(fx(*x1), fy(&map, *y1), fx(*x2), fy(&map, *y2), fx(*x), fy(&map, *y)));
                    last = Some((*x, *y, out.len() - 1));
                }
                Op::Close => {
                    if let Some((x, y, k)) = last
                        && (x, y) == start
                    {
                        let sy = fy(&first, y);
                        match &mut out[k] {
                            Seg::Line(_, py) => *py = sy,
                            Seg::Cubic(_, _, _, _, _, py) => *py = sy,
                            _ => {}
                        }
                    }
                    out.push(Seg::Close);
                    last = None;
                }
            }
        }
        Some(out)
    }
}
