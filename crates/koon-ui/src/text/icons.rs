use super::icon_table::ICONS;
use kurbo::{BezPath, Circle, Ellipse, PathEl, Point, Rect, RoundedRect, Shape};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub struct Icon {
    pub name: &'static str,
    pub shapes: Vec<(BezPath, bool)>,
}

fn attrs(tag: &str) -> HashMap<&str, &str> {
    let mut out = HashMap::new();
    let mut rest = tag;
    while let Some(eq) = rest.find("=\"") {
        let key = rest[..eq].rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
        let after = &rest[eq + 2..];
        let Some(end) = after.find('"') else { break };
        out.insert(key, &after[..end]);
        rest = &after[end + 1..];
    }
    out
}

fn num(a: &HashMap<&str, &str>, k: &str) -> f64 {
    a.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0)
}

pub fn parse(name: &'static str, svg: &str) -> Icon {
    let mut shapes = Vec::new();
    let body = svg.split_once('>').map(|(_, b)| b).unwrap_or(svg);
    for part in body.split('<').skip(1) {
        let part = part.trim_end_matches(|c: char| c.is_whitespace() || c == '>' || c == '/');
        let tag = part.split_whitespace().next().unwrap_or("");
        let a = attrs(part);
        let fill = a.get("fill").is_some_and(|f| *f == "currentColor");
        let path = match tag {
            "path" => BezPath::from_svg(a.get("d").copied().unwrap_or("")).unwrap_or_default(),
            "circle" => Circle::new((num(&a, "cx"), num(&a, "cy")), num(&a, "r")).to_path(0.01),
            "ellipse" => Ellipse::new((num(&a, "cx"), num(&a, "cy")), (num(&a, "rx"), num(&a, "ry")), 0.0).to_path(0.01),
            "rect" => {
                let (x, y, w, h) = (num(&a, "x"), num(&a, "y"), num(&a, "width"), num(&a, "height"));
                let rx = if a.contains_key("rx") { num(&a, "rx") } else { num(&a, "ry") };
                RoundedRect::from_rect(Rect::new(x, y, x + w, y + h), rx.min(w / 2.0).min(h / 2.0)).to_path(0.01)
            }
            "line" => {
                let mut p = BezPath::new();
                p.move_to((num(&a, "x1"), num(&a, "y1")));
                p.line_to((num(&a, "x2"), num(&a, "y2")));
                p
            }
            "polyline" | "polygon" => {
                let pts: Vec<f64> = a.get("points").unwrap_or(&"").split(|c: char| c == ',' || c.is_whitespace()).filter_map(|s| s.parse().ok()).collect();
                let mut p = BezPath::new();
                for (i, c) in pts.chunks(2).enumerate() {
                    if c.len() == 2 {
                        let pt = Point::new(c[0], c[1]);
                        if i == 0 {
                            p.move_to(pt);
                        } else {
                            p.line_to(pt);
                        }
                    }
                }
                if tag == "polygon" {
                    p.close_path();
                }
                p
            }
            _ => continue,
        };
        shapes.push((path, fill));
    }
    Icon { name, shapes }
}

pub fn get(name: &str) -> Option<&'static Icon> {
    static ALL: OnceLock<Mutex<HashMap<&'static str, &'static Icon>>> = OnceLock::new();
    let i = ICONS.binary_search_by(|(n, _)| (*n).cmp(name)).ok()?;
    let (n, svg) = ICONS[i];
    let mut all = ALL.get_or_init(Default::default).lock().unwrap();
    Some(*all.entry(n).or_insert_with(|| Box::leak(Box::new(parse(n, svg)))))
}

pub fn names() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(n, _)| *n)
}

fn skia(p: &BezPath) -> Option<tiny_skia::Path> {
    let mut b = PathBuilder::new();
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(a) => b.move_to(a.x as f32, a.y as f32),
            PathEl::LineTo(a) => b.line_to(a.x as f32, a.y as f32),
            PathEl::QuadTo(a, c) => b.quad_to(a.x as f32, a.y as f32, c.x as f32, c.y as f32),
            PathEl::CurveTo(a, c, d) => b.cubic_to(a.x as f32, a.y as f32, c.x as f32, c.y as f32, d.x as f32, d.y as f32),
            PathEl::ClosePath => b.close(),
        }
    }
    b.finish()
}

pub fn render(icon: &Icon, width: u32, height: u32, tx: f32, ty: f32, scale: f32, line: f32, fill: bool) -> Vec<u8> {
    let Some(mut pm) = Pixmap::new(width.max(1), height.max(1)) else {
        return vec![0; (width * height) as usize];
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: line,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let t = Transform::from_row(scale, 0.0, 0.0, scale, tx, ty);
    for (p, f) in &icon.shapes {
        let Some(path) = skia(p) else { continue };
        if fill || *f {
            pm.fill_path(&path, &paint, FillRule::Winding, t, None);
        }
        pm.stroke_path(&path, &paint, &stroke, t, None);
    }
    pm.pixels().iter().map(|p| p.alpha()).collect()
}
