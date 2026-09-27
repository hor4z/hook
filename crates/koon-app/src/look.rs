use koon_core::{Kind, Mark, Status};
use koon_ui::{Ease, Face, R, Rgba, Ui, alpha, hex, id};
use std::f32::consts::TAU;

pub const LOGO: &str = include_str!("../../../assets/logo.svg");
pub const PILL: (f32, f32) = (52.0, 104.0);
pub const MARGIN: f32 = 12.0;
pub const DOCK_W: f32 = 52.0;

pub fn accent() -> Rgba {
    hex("#86D94F", 1.0)
}

pub fn blue() -> Rgba {
    hex("#6AAEFF", 1.0)
}

pub fn surface() -> Rgba {
    hex("#141414", 0.96)
}

pub fn edge() -> Rgba {
    [1.0, 1.0, 1.0, 0.09]
}

pub fn ink() -> Rgba {
    [1.0, 1.0, 1.0, 0.94]
}

pub fn muted() -> Rgba {
    [1.0, 1.0, 1.0, 0.52]
}

pub fn dark() -> Rgba {
    hex("#0C0F0A", 1.0)
}

#[derive(Default)]
pub struct PillLook {
    pub level: f32,
    pub listening: bool,
    pub busy: bool,
    pub hidden: bool,
    pub open: usize,
    pub working: bool,
    pub shadow: f32,
}

pub fn pill(ui: &mut Ui, v: &PillLook) -> R {
    let r = R::new(MARGIN, MARGIN, PILL.0, PILL.1);
    if v.hidden {
        return r;
    }
    let key = id(&["pill"]);
    pill_shadow(ui, r, v.shadow);
    if v.shadow < 1.0 {
        ui.s.animating = true;
    }
    ui.rect(r, PILL.0 / 2.0, surface());
    let cx = r.x + r.w / 2.0;
    ui.svg("logo", LOGO, logo_rect(r));
    bars(ui, cx, r.bottom() - 26.0, v, 1.0);
    let badge = ui.anim(id(&["pill", "badge"]), if v.open > 0 { 1.0 } else { 0.0 }, 260.0, Ease::Bezier(0.34, 1.56, 0.64, 1.0));
    if badge > 0.01 {
        let n = v.open.max(1).to_string();
        let c = if v.working { blue() } else { accent() };
        let d = 18.0 * badge;
        let (bx, by) = (r.right() - 8.0, r.y + 6.0);
        let b = R::new(bx - d / 2.0, by - d / 2.0, d, d);
        ui.rect(b, d / 2.0, c);
        ui.border(b, d / 2.0, surface(), 2.0);
        let w = ui.measure(&n, Face::Sans600, 10.0 * badge);
        ui.centered(bx - w / 2.0, by, &n, Face::Sans600, 10.0 * badge, dark());
        if v.working {
            let t = (ui.now / 1000.0) as f32 * TAU * 0.9;
            ui.arc((bx, by), d / 2.0 + 3.0, t, TAU * 0.3, 1.5, blue());
            ui.s.animating = true;
        }
    }
    ui.region(r, key);
    if ui.hovered(key) {
        ui.cursor(koon_ui::Cursor::Pointer);
    }
    r
}

pub fn bars(ui: &mut Ui, cx: f32, cy: f32, v: &PillLook, k: f32) {
    let t = (ui.now / 1000.0) as f32;
    for i in 0..3 {
        let f = i as f32;
        let h = if v.busy {
            4.0 + 8.0 * ((t * 7.0 - f * 0.9).sin() * 0.5 + 0.5)
        } else if v.listening {
            let wobble = (t * (9.0 + f * 2.3) + f * 1.7).sin() * 0.35 + 0.65;
            let target = 4.0 + 14.0 * (v.level * wobble).clamp(0.0, 1.0);
            ui.anim(id(&["bar", &i.to_string()]), target, 90.0, Ease::OutCubic)
        } else {
            ui.anim(id(&["bar", &i.to_string()]), 4.0, 220.0, Ease::Css)
        };
        let h = h * k;
        let x = cx + (f - 1.0) * 8.0 * k - 2.0 * k;
        ui.rect(R::new(x, cy - h / 2.0, 4.0 * k, h), 2.0 * k, accent());
    }
    if v.busy || v.listening {
        ui.s.animating = true;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Comment,
    Draw,
    Area,
    SaveMemory,
    Memories,
    Voice,
    Clear,
    Grab,
}

pub const ITEMS: [(Act, &str, &str, &str); 7] = [
    (Act::Comment, "message-circle-plus", "Comentar", "C"),
    (Act::Draw, "pencil", "Dibujar", "D"),
    (Act::Area, "square-dashed", "Marcar área", "A"),
    (Act::SaveMemory, "bookmark-plus", "Guardar memory", "M"),
    (Act::Memories, "gallery-thumbnails", "Memories", "⌃⌥M"),
    (Act::Voice, "mic", "Dictar comentario", "V"),
    (Act::Clear, "eraser", "Descartar todo", ""),
];

pub struct DockLook {
    pub at: (f32, f32),
    pub up: bool,
    pub area: R,
    pub open: bool,
    pub linger: Option<f32>,
    pub active: Option<Act>,
}

const ITEMS_H: f32 = 40.0 * ITEMS.len() as f32 + 8.0;

pub fn dock_height(up: bool) -> f32 {
    if up { 20.0 + ITEMS_H + PILL.1 } else { 56.0 + ITEMS_H + 16.0 }
}

pub fn pill_body(ui: &mut Ui, r: R) {
    pill_shadow(ui, r, 1.0);
    ui.rect(r, PILL.0 / 2.0, surface());
}

fn pill_shadow(ui: &mut Ui, r: R, k: f32) {
    if k > 0.0 {
        ui.shadow(r, PILL.0 / 2.0, 9.0, 0.0, 3.0, [0.0, 0.0, 0.0, 0.42 * k.min(1.0)]);
    }
}

pub fn logo_rect(pill: R) -> R {
    let cx = pill.x + pill.w / 2.0;
    R::new((cx - 13.0).round(), (pill.y + 22.0).round(), 26.0, 26.0)
}

pub fn dock(ui: &mut Ui, v: &DockLook) -> Option<Act> {
    let open = ui.anim(id(&["dock", "open"]), if v.open { 1.0 } else { 0.0 }, 320.0, Ease::Snap);
    let dots = ui.anim_delay(
        id(&["dock", "bars"]),
        if v.open { 0.0 } else { 1.0 },
        if v.open { 90.0 } else { 200.0 },
        if v.open { 0.0 } else { 100.0 },
        Ease::Css,
    );
    if open <= 0.001 && !v.open {
        for (_, _, label, _) in ITEMS.iter() {
            ui.set_anim(id(&["dock", "item", label]), 0.0);
        }
        ui.set_anim(id(&["dock", "sep"]), 0.0);
        if let Some(k) = v.linger {
            let pill = R::new(v.at.0, v.at.1, PILL.0, PILL.1);
            pill_shadow(ui, pill, k);
            ui.rect(pill, PILL.0 / 2.0, surface());
            if k < 1.0 {
                ui.s.animating = true;
            }
            ui.svg("logo", LOGO, logo_rect(pill));
            ui.push_alpha(dots);
            bars(ui, pill.x + pill.w / 2.0, pill.bottom() - 26.0, &PillLook::default(), 1.0);
            ui.pop_alpha();
        }
        return None;
    }
    let full = dock_height(v.up);
    let h = PILL.1 + (full - PILL.1) * open;
    let (x, y) = v.at;
    let pill = R::new(x, y, PILL.0, PILL.1);
    let top = if v.up { y + PILL.1 - h } else { y };
    let r = R::new(x, top, DOCK_W, h);
    pill_body(ui, r);
    ui.region(r, id(&["dock"]));
    let cx = x + DOCK_W / 2.0;
    ui.svg("logo", LOGO, logo_rect(pill));
    ui.push_alpha(dots);
    bars(ui, cx, pill.bottom() - 26.0, &PillLook::default(), 1.0);
    ui.pop_alpha();
    let mut out = None;
    let logo = id(&["dock", "logo"]);
    let head = R::new(x, y, DOCK_W, 56.0);
    ui.region(head, logo);
    if ui.hovered(logo) {
        ui.cursor(koon_ui::Cursor::Pointer);
    }
    if ui.pressed_on(logo) {
        out = Some(Act::Grab);
    }
    let mut tips = None;
    ui.clip(r);
    let mut iy = if v.up { top + 12.0 } else { y + 56.0 };
    for (i, (act, icon, label, kbd)) in ITEMS.iter().enumerate() {
        if *act == Act::Clear {
            let s = ui.anim_delay(id(&["dock", "sep"]), if v.open { 1.0 } else { 0.0 }, 200.0, 30.0 * i as f64, Ease::Snap);
            ui.rect(R::new(cx - 10.0 * s, iy + 2.0, 20.0 * s, 1.0), 0.5, edge());
            iy += 8.0;
        }
        let key = id(&["dock", label]);
        let t = ui.anim_delay(
            id(&["dock", "item", label]),
            if v.open { 1.0 } else { 0.0 },
            260.0,
            if v.open { 60.0 + 26.0 * (if v.up { ITEMS.len() - 1 - i } else { i }) as f64 } else { 0.0 },
            Ease::Snap,
        );
        if t <= 0.01 {
            iy += 40.0;
            continue;
        }
        let cell = R::new(cx - 18.0, iy + 2.0, 36.0, 36.0);
        let disabled = false;
        let hot = ui.hovered(key) && !disabled;
        let on = v.active == Some(*act);
        let hover = ui.anim(id(&["dock", "hover", label]), if hot { 1.0 } else { 0.0 }, 140.0, Ease::Css);
        ui.push_alpha(t);
        ui.push_offset(0.0, (1.0 - t) * if v.up { 8.0 } else { -8.0 });
        if on || hover > 0.0 {
            ui.rect(cell, 12.0, [1.0, 1.0, 1.0, if on { 0.12 } else { 0.07 * hover }]);
        }
        let c = if disabled {
            [1.0, 1.0, 1.0, 0.25]
        } else if on {
            accent()
        } else {
            koon_ui::mix(muted(), ink(), hover)
        };
        ui.icon(icon, cell.x + 9.0, cell.y + 9.0, 18.0, c);
        ui.pop_offset();
        ui.pop_alpha();
        ui.region(cell, key);
        if hot {
            ui.cursor(koon_ui::Cursor::Pointer);
            tips = Some((key, cell, *label, *kbd));
        }
        if !disabled && ui.clicked(key) {
            out = Some(*act);
        }
        iy += 40.0;
    }
    ui.unclip();
    if let Some((key, cell, label, kbd)) = tips {
        tip(ui, key, cell, label, kbd, x, v.area);
    }
    out
}

fn tip(ui: &mut Ui, key: u64, cell: R, label: &str, kbd: &str, dock_x: f32, area: R) {
    let t = ui.anim_delay(id(&["tip", &key.to_string()]), 1.0, 160.0, 220.0, Ease::Snap);
    if t <= 0.01 {
        return;
    }
    let size = 12.0;
    let w = ui.measure(label, Face::Sans500, size) + if kbd.is_empty() { 20.0 } else { 20.0 + ui.measure(kbd, Face::Mono500, 11.0) + 12.0 };
    let left = dock_x + DOCK_W + 10.0 + w > area.right() - 8.0 || (dock_x > area.x + area.w / 2.0 && dock_x - 10.0 - w >= area.x + 8.0);
    let x = if left { dock_x - 10.0 - w } else { dock_x + DOCK_W + 10.0 };
    let r = R::new(x + if left { 6.0 } else { -6.0 } * (1.0 - t), cell.y + 5.0, w, 26.0);
    ui.push_alpha(t);
    ui.shadow(r, 8.0, 12.0, 0.0, 4.0, [0.0, 0.0, 0.0, 0.35]);
    ui.rect(r, 8.0, surface());
    ui.border(r, 8.0, edge(), 1.0);
    let end = ui.centered(r.x + 10.0, r.y + 13.0, label, Face::Sans500, size, ink());
    if !kbd.is_empty() {
        ui.centered(end + 12.0, r.y + 13.0, kbd, Face::Mono500, 11.0, muted());
    }
    ui.pop_alpha();
}

pub fn mark_color(m: &Mark) -> Rgba {
    if m.status == Status::Taken { blue() } else { accent() }
}

pub fn badge_rect(m: &Mark) -> R {
    let [x, y] = m.anchor();
    match m.kind {
        Kind::Pin => R::new(x, y - 26.0, 26.0, 26.0),
        Kind::Stroke => R::new(x + 4.0, y - 26.0, 26.0, 26.0),
        Kind::Area => R::new(x - 13.0, y - 13.0, 26.0, 26.0),
    }
}

pub fn mark(ui: &mut Ui, m: &Mark, a: f32, focus: bool, label: bool) {
    if a <= 0.01 {
        return;
    }
    let c = mark_color(m);
    ui.push_alpha(a);
    match m.kind {
        Kind::Stroke => {
            for w in m.points.windows(2) {
                ui.segment((w[0][0], w[0][1]), (w[1][0], w[1][1]), 6.0, [0.0, 0.0, 0.0, 0.18]);
            }
            for w in m.points.windows(2) {
                ui.segment((w[0][0], w[0][1]), (w[1][0], w[1][1]), 3.5, c);
            }
        }
        Kind::Area => {
            let b = m.bounds();
            let r = R::new(b[0], b[1], b[2], b[3]);
            ui.rect(r, 6.0, alpha(c, 0.10));
            ui.border(r, 6.0, [0.0, 0.0, 0.0, 0.20], 3.0);
            ui.border(r, 6.0, c, 2.0);
        }
        Kind::Pin => {}
    }
    let pop = ui.appear(
        id(&["mark", "pop", &m.id.to_string(), &format!("{:.0}{:.0}", m.points[0][0], m.points[0][1])]),
        280.0,
        0.0,
        Ease::Bezier(0.34, 1.56, 0.64, 1.0),
    );
    let b = badge_rect(m);
    let grow = ui.anim(id(&["mark", "focus", &m.id.to_string()]), if focus { 1.0 } else { 0.0 }, 180.0, Ease::OutCubic);
    let s = (0.6 + 0.4 * pop) * (1.0 + 0.14 * grow);
    let br = R::new(
        b.x + b.w * (1.0 - s) * if m.kind == Kind::Area { 0.5 } else { 0.0 },
        b.bottom() - b.h * s - if m.kind == Kind::Area { b.h * (1.0 - s) * 0.5 } else { 0.0 },
        b.w * s,
        b.h * s,
    );
    let radii = if m.kind == Kind::Area { [13.0 * s; 4] } else { [13.0 * s, 13.0 * s, 13.0 * s, 3.0] };
    ui.shadow(br, 13.0, 10.0, 0.0, 3.0, [0.0, 0.0, 0.0, 0.35]);
    ui.rect_radii(br, radii, c);
    ui.border_radii(br, radii, [1.0, 1.0, 1.0, if focus { 0.95 } else { 0.75 }], if focus { 2.5 } else { 2.0 });
    let n = if m.status != Status::Resolved && m.asking().is_some() {
        "?".to_string()
    } else {
        m.id.to_string()
    };
    let w = ui.measure(&n, Face::Sans600, 12.0 * s);
    ui.centered(br.x + (br.w - w) / 2.0, br.y + br.h / 2.0, &n, Face::Sans600, 12.0 * s, dark());
    if m.status == Status::Taken && m.asking().is_none() {
        let t = (ui.now / 1000.0) as f32 * TAU * 0.9;
        ui.arc((br.x + br.w / 2.0, br.y + br.h / 2.0), br.w / 2.0 + 4.0, t, TAU * 0.3, 2.0, blue());
        ui.s.animating = true;
    }
    if label {
        self::label(ui, m);
    }
    ui.pop_alpha();
}

fn wrap(ui: &mut Ui, text: &str, face: Face, size: f32, max: f32, lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if ui.measure(&next, face, size) <= max || line.is_empty() {
            line = next;
        } else {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.len() > lines {
        let rest = out[lines - 1..].join(" ");
        out.truncate(lines - 1);
        out.push(ui.fit(&rest, face, size, max));
    }
    out
}

pub fn thread(ui: &mut Ui, key: u64, anchor: R, bounds: R, m: &Mark, text: &mut String) -> Option<BubbleOut> {
    let t = ui.appear(id(&["thread", &key.to_string()]), 220.0, 0.0, Ease::Snap);
    let w = 340.0;
    let inner = w - 28.0;
    let mut msgs: Vec<(bool, Vec<String>)> = std::iter::once((true, m.text.clone()))
        .chain(m.thread.iter().map(|e| (e.author == koon_core::Author::User, e.text.clone())))
        .filter(|(_, t)| !t.trim().is_empty())
        .map(|(u, t)| (u, wrap(ui, &t, Face::Sans400, 13.0, inner - 12.0, 4)))
        .collect();
    let height = |msgs: &[(bool, Vec<String>)]| msgs.iter().map(|(_, l)| 22.0 + 18.0 * l.len() as f32 + 8.0).sum::<f32>();
    while msgs.len() > 1 && height(&msgs) > 300.0 {
        msgs.remove(0);
    }
    let h = 14.0 + height(&msgs) + 90.0;
    let mut x = anchor.right() + 10.0;
    if x + w > bounds.right() - 12.0 {
        x = anchor.x - 10.0 - w;
    }
    let y = (anchor.y - 6.0).clamp(bounds.y + 12.0, (bounds.bottom() - h - 12.0).max(bounds.y + 12.0));
    let r = R::new(x.max(bounds.x + 12.0), y + (1.0 - t) * 6.0, w, h);
    ui.push_alpha(t);
    ui.shadow(r, 14.0, 26.0, 0.0, 10.0, [0.0, 0.0, 0.0, 0.45]);
    ui.rect(r, 14.0, surface());
    ui.border(r, 14.0, edge(), 1.0);
    ui.region(r, id(&["bubble", &key.to_string()]));
    let mut cy = r.y + 14.0;
    for (user, lines) in &msgs {
        let c = if *user { accent() } else { blue() };
        let bh = 22.0 + 18.0 * lines.len() as f32;
        ui.rect(R::new(r.x + 14.0, cy, 2.0, bh - 4.0), 1.0, alpha(c, 0.8));
        let b = ui.baseline(Face::Sans600, 11.0, cy + 8.0);
        ui.text(r.x + 24.0, b, if *user { "Vos" } else { "Agente" }, Face::Sans600, 11.0, c);
        for (i, l) in lines.iter().enumerate() {
            ui.centered(r.x + 24.0, cy + 27.0 + 18.0 * i as f32, l, Face::Sans400, 13.0, ink());
        }
        cy += bh + 8.0;
    }
    ui.rect(R::new(r.x + 14.0, cy, r.w - 28.0, 1.0), 0.0, edge());
    let field_key = id(&["thread", "field", &key.to_string()]);
    let field = R::new(r.x + 14.0, cy + 8.0, r.w - 28.0, 34.0);
    ui.s.focus = Some(field_key);
    let placeholder = if m.asking().is_some() { "Respondé al agente…" } else { "Responder…" };
    let res = koon_ui::widgets::text_field(ui, field_key, field, text, placeholder, Face::Sans400, 14.0, ink());
    let hint = ui.baseline(Face::Sans400, 11.0, r.bottom() - 22.0);
    let end = ui.text(r.x + 14.0, hint, "↵", Face::Mono500, 11.0, ink());
    let end = ui.text(end + 5.0, hint, "enviar", Face::Sans400, 11.0, muted());
    let end = ui.text(end + 12.0, hint, "esc", Face::Mono500, 11.0, ink());
    ui.text(end + 5.0, hint, "cerrar", Face::Sans400, 11.0, muted());
    let del = id(&["thread", "delete", &key.to_string()]);
    let cell = R::new(r.right() - 40.0, r.bottom() - 35.0, 26.0, 26.0);
    let hover = ui.hovered(del);
    if hover {
        ui.rect(cell, 8.0, [1.0, 1.0, 1.0, 0.08]);
        ui.cursor(koon_ui::Cursor::Pointer);
    }
    ui.icon("trash-2", cell.x + 5.0, cell.y + 5.0, 16.0, if hover { hex("#FF7A6B", 1.0) } else { muted() });
    ui.pop_alpha();
    ui.region(cell, del);
    if ui.clicked(del) {
        return Some(BubbleOut::Delete);
    }
    if res.submitted.is_some() {
        return Some(BubbleOut::Save);
    }
    if ui.key(koon_ui::Key::Escape).is_some() {
        return Some(BubbleOut::Cancel);
    }
    None
}

pub fn ask(ui: &mut Ui, m: &Mark, question: &str) {
    let br = badge_rect(m);
    let lines = wrap(ui, question.trim(), Face::Sans500, 12.0, 360.0, 2);
    let w = lines.iter().map(|l| ui.measure(l, Face::Sans500, 12.0)).fold(0.0, f32::max) + 42.0;
    let h = 12.0 + 18.0 * lines.len() as f32;
    let r = R::new(br.right() + 6.0, br.y + (br.h - 26.0) / 2.0, w, h);
    ui.shadow(r, 9.0, 12.0, 0.0, 4.0, [0.0, 0.0, 0.0, 0.35]);
    ui.rect(r, 9.0, surface());
    ui.border(r, 9.0, alpha(blue(), 0.7), 1.0);
    ui.icon("message-circle-question", r.x + 10.0, r.y + 7.0, 14.0, blue());
    for (i, l) in lines.iter().enumerate() {
        ui.centered(r.x + 30.0, r.y + 15.0 + 18.0 * i as f32, l, Face::Sans500, 12.0, ink());
    }
}

pub fn done(ui: &mut Ui, m: &Mark, note: &str, hot: bool) -> R {
    let br = badge_rect(m);
    let text = ui.fit(note.trim(), Face::Sans500, 12.0, 280.0);
    let extra = "no, así no";
    let more = if hot { ui.measure(extra, Face::Sans500, 12.0) + 22.0 } else { 0.0 };
    let w = ui.measure(&text, Face::Sans500, 12.0) + 40.0 + more;
    let r = R::new(br.right() + 6.0, br.y + (br.h - 26.0) / 2.0, w, 26.0);
    ui.rect(r, 9.0, surface());
    ui.border(r, 9.0, alpha(accent(), if hot { 0.9 } else { 0.55 }), 1.0);
    ui.icon("check", r.x + 9.0, r.y + 6.0, 14.0, accent());
    let end = ui.centered(r.x + 28.0, r.y + 13.0, &text, Face::Sans500, 12.0, ink());
    if hot {
        ui.rect(R::new(end + 9.0, r.y + 7.0, 1.0, 12.0), 0.0, edge());
        ui.centered(end + 19.0, r.y + 13.0, extra, Face::Sans500, 12.0, muted());
    }
    r
}

pub fn label(ui: &mut Ui, m: &Mark) {
    if m.text.trim().is_empty() {
        return;
    }
    let br = badge_rect(m);
    let text = ui.fit(m.text.trim(), Face::Sans500, 12.0, 240.0);
    let w = ui.measure(&text, Face::Sans500, 12.0) + 20.0;
    let r = R::new(br.right() + 6.0, br.y + (br.h - 24.0) / 2.0, w, 24.0);
    ui.rect(r, 8.0, surface());
    ui.border(r, 8.0, edge(), 1.0);
    ui.centered(r.x + 10.0, r.y + 12.0, &text, Face::Sans500, 12.0, ink());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BubbleOut {
    Save,
    SaveNext,
    Cancel,
    Delete,
    Mic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mic {
    Off,
    Listening(f32),
    Busy(&'static str),
}

pub fn field_key(key: u64) -> u64 {
    id(&["bubble", "field", &key.to_string()])
}

fn voice_bars(ui: &mut Ui, x: f32, cy: f32, level: Option<f32>, tint: Rgba) -> f32 {
    let t = (ui.now / 1000.0) as f32;
    for i in 0..5 {
        let f = i as f32;
        let h = match level {
            Some(l) => {
                let wobble = (t * (8.0 + f * 1.9) + f * 1.3).sin() * 0.3 + 0.7;
                let shape = 1.0 - (f - 2.0).abs() * 0.18;
                ui.anim(id(&["voice", "bar", &i.to_string()]), 3.0 + 13.0 * (l * wobble * shape).clamp(0.0, 1.0), 80.0, Ease::OutCubic)
            }
            None => 3.0 + 6.0 * ((t * 6.0 - f * 0.8).sin() * 0.5 + 0.5),
        };
        ui.rect(R::new(x + f * 5.0, cy - h / 2.0, 3.0, h), 1.5, tint);
    }
    ui.s.animating = true;
    x + 5.0 * 5.0 + 4.0
}

#[allow(clippy::too_many_arguments)]
pub fn bubble(ui: &mut Ui, key: u64, anchor: R, bounds: R, text: &mut String, placeholder: &str, mic: Mic, tint: Rgba) -> Option<BubbleOut> {
    let t = ui.appear(id(&["bubble", &key.to_string()]), 220.0, 0.0, Ease::Snap);
    let (w, h) = (320.0, 84.0);
    let mut x = anchor.right() + 10.0;
    if x + w > bounds.right() - 12.0 {
        x = anchor.x - 10.0 - w;
    }
    let y = (anchor.y - 6.0).clamp(bounds.y + 12.0, bounds.bottom() - h - 12.0);
    let r = R::new(x.max(bounds.x + 12.0), y + (1.0 - t) * 6.0, w, h);
    ui.push_alpha(t);
    ui.shadow(r, 14.0, 26.0, 0.0, 10.0, [0.0, 0.0, 0.0, 0.45]);
    ui.rect(r, 14.0, surface());
    ui.border(r, 14.0, edge(), 1.0);
    ui.region(r, id(&["bubble", &key.to_string()]));
    let field_key = field_key(key);
    let field = R::new(r.x + 14.0, r.y + 10.0, r.w - 28.0, 34.0);
    ui.s.focus = Some(field_key);
    let res = koon_ui::widgets::text_field(ui, field_key, field, text, placeholder, Face::Sans400, 14.0, ink());
    ui.rect(R::new(r.x + 14.0, r.y + 46.0, r.w - 28.0, 1.0), 0.0, edge());
    let hint = ui.baseline(Face::Sans400, 11.0, r.y + 65.0);
    match mic {
        Mic::Off => {
            let end = ui.text(r.x + 14.0, hint, "↵", Face::Mono500, 11.0, ink());
            let end = ui.text(end + 5.0, hint, "guardar", Face::Sans400, 11.0, muted());
            let end = ui.text(end + 12.0, hint, "⇧↵", Face::Mono500, 11.0, ink());
            let end = ui.text(end + 5.0, hint, "y seguir", Face::Sans400, 11.0, muted());
            let end = ui.text(end + 12.0, hint, "esc", Face::Mono500, 11.0, ink());
            ui.text(end + 5.0, hint, "cancelar", Face::Sans400, 11.0, muted());
        }
        Mic::Listening(level) => {
            let end = voice_bars(ui, r.x + 14.0, r.y + 61.0, Some(level), tint);
            let end = ui.text(end + 4.0, hint, "Escuchando…", Face::Sans400, 11.0, ink());
            let end = ui.text(end + 12.0, hint, "↵", Face::Mono500, 11.0, ink());
            ui.text(end + 5.0, hint, "listo", Face::Sans400, 11.0, muted());
        }
        Mic::Busy(label) => {
            let end = voice_bars(ui, r.x + 14.0, r.y + 61.0, None, tint);
            ui.text(end + 4.0, hint, label, Face::Sans400, 11.0, muted());
        }
    }
    let mic_key = id(&["bubble", "mic", &key.to_string()]);
    let mic_cell = R::new(r.right() - 70.0, r.y + 52.0, 26.0, 26.0);
    let mic_hover = ui.hovered(mic_key);
    let live = matches!(mic, Mic::Listening(_));
    if live {
        ui.rect(mic_cell, 8.0, koon_ui::alpha(tint, 0.16));
    } else if mic_hover {
        ui.rect(mic_cell, 8.0, [1.0, 1.0, 1.0, 0.08]);
    }
    if mic_hover {
        ui.cursor(koon_ui::Cursor::Pointer);
    }
    let mic_color = if mic != Mic::Off {
        tint
    } else if mic_hover {
        ink()
    } else {
        muted()
    };
    ui.icon("mic", mic_cell.x + 5.0, mic_cell.y + 5.0, 16.0, mic_color);
    let del = id(&["bubble", "delete", &key.to_string()]);
    let cell = R::new(r.right() - 40.0, r.y + 52.0, 26.0, 26.0);
    let hover = ui.hovered(del);
    if hover {
        ui.rect(cell, 8.0, [1.0, 1.0, 1.0, 0.08]);
        ui.cursor(koon_ui::Cursor::Pointer);
    }
    ui.icon("trash-2", cell.x + 5.0, cell.y + 5.0, 16.0, if hover { hex("#FF7A6B", 1.0) } else { muted() });
    ui.pop_alpha();
    ui.region(cell, del);
    ui.region(mic_cell, mic_key);
    if ui.clicked(mic_key) && !matches!(mic, Mic::Busy(_)) {
        return Some(BubbleOut::Mic);
    }
    if ui.clicked(del) {
        return Some(BubbleOut::Delete);
    }
    if let Some(m) = res.submitted {
        return Some(if m.shift || m.ctrl { BubbleOut::SaveNext } else { BubbleOut::Save });
    }
    if ui.key(koon_ui::Key::Escape).is_some() {
        return Some(BubbleOut::Cancel);
    }
    None
}

pub struct Card {
    pub id: String,
    pub name: String,
    pub note: String,
    pub path: String,
    pub thumb: Option<koon_core::Image>,
    pub full: Option<std::path::PathBuf>,
}

pub struct Palette {
    pub cards: Vec<Card>,
    pub query: String,
    pub sel: usize,
    pub asking: Option<usize>,
    pub text: String,
    pub doomed: Option<String>,
    pub dir: String,
}

pub enum PaletteOut {
    Close,
    Delete(String),
    Use(String, String),
}

pub struct Folder {
    pub name: String,
    pub path: String,
    pub count: usize,
    pub peek: Vec<usize>,
}

impl Palette {
    pub fn new(cards: Vec<Card>) -> Palette {
        Palette {
            cards,
            query: String::new(),
            sel: 0,
            asking: None,
            text: String::new(),
            doomed: None,
            dir: String::new(),
        }
    }

    pub fn visible(&self) -> Vec<usize> {
        let q = self.query.trim().to_lowercase();
        (0..self.cards.len())
            .filter(|&i| {
                let c = &self.cards[i];
                if q.is_empty() {
                    return c.path == self.dir;
                }
                c.name.to_lowercase().contains(&q) || c.id.contains(&q) || c.note.to_lowercase().contains(&q) || c.path.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn folders(&self) -> Vec<Folder> {
        if !self.query.trim().is_empty() {
            return Vec::new();
        }
        let mut out: Vec<Folder> = Vec::new();
        for (i, c) in self.cards.iter().enumerate() {
            let rest = if self.dir.is_empty() {
                c.path.as_str()
            } else {
                match c.path.strip_prefix(&self.dir).and_then(|r| r.strip_prefix('/')) {
                    Some(r) => r,
                    None => continue,
                }
            };
            let Some(name) = rest.split('/').next().filter(|n| !n.is_empty()) else { continue };
            let path = if self.dir.is_empty() { name.to_string() } else { format!("{}/{name}", self.dir) };
            match out.iter_mut().find(|f| f.path == path) {
                Some(f) => {
                    f.count += 1;
                    if f.peek.len() < 3 {
                        f.peek.push(i);
                    }
                }
                None => out.push(Folder {
                    name: name.to_string(),
                    path,
                    count: 1,
                    peek: vec![i],
                }),
            }
        }
        out.sort_by_key(|f| f.name.to_lowercase());
        out
    }

    fn up(&mut self) {
        self.dir = self.dir.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default();
        self.sel = 0;
        self.doomed = None;
    }
}

fn thumb(ui: &mut Ui, c: &Card, r: R) {
    ui.rect(r, 8.0, [1.0, 1.0, 1.0, 0.05]);
    let Some(img) = c.thumb.as_ref() else { return };
    let s = (r.w / img.w as f32).min(r.h / img.h as f32);
    let (w, h) = (img.w as f32 * s, img.h as f32 * s);
    let fit = R::new(r.x + (r.w - w) / 2.0, r.y + (r.h - h) / 2.0, w, h);
    ui.bitmap(&format!("memory|{}|{}x{}", c.id, img.w, img.h), img.w, img.h, &img.rgba, fit);
    ui.border(r, 8.0, edge(), 1.0);
}

const FOLDERS: [&str; 6] = ["#7AA7FF", "#6FD69A", "#FF8A5C", "#FFD84D", "#F5A3E6", "#8FD6FF"];
const FOLDER_BACK: &str = "M12 0h34c8 0 10 1.5 14 5c4 3.5 8 4 16 4h42a12 12 0 0 1 12 12v67a12 12 0 0 1-12 12H12A12 12 0 0 1 0 88V12A12 12 0 0 1 12 0z";

fn folder(ui: &mut Ui, p: &Palette, f: &Folder, r: R, hover: f32, k: usize) {
    let top = hex(FOLDERS[k % FOLDERS.len()], 1.0);
    let back = koon_ui::mix(top, [0.0, 0.0, 0.0, 1.0], 0.16);
    let (w, h) = (r.w, r.w / 1.3);
    let body = R::new(r.x, r.y + r.h - h, w, h);
    let tab = h * 0.17;
    let radius = w * 0.09;
    let fill = format!("#{:02X}{:02X}{:02X}", (back[0] * 255.0) as u8, (back[1] * 255.0) as u8, (back[2] * 255.0) as u8);
    let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 130 100\"><path fill=\"{fill}\" d=\"{FOLDER_BACK}\"/></svg>");
    ui.svg(&format!("folder|{fill}"), &svg, body);
    let spread: &[(f32, f32)] = match f.peek.len() {
        1 => &[(0.0, -0.42)],
        2 => &[(-0.13, -0.36), (0.13, -0.36)],
        _ => &[(-0.20, -0.30), (0.0, -0.42), (0.20, -0.30)],
    };
    let (sw, sh) = (w * 0.56, h * 0.56);
    for (k, &i) in f.peek.iter().enumerate() {
        let (dx, dy) = spread[k.min(spread.len() - 1)];
        let (rx, ry) = (dx * 0.35, -0.14);
        let ox = (rx + (dx - rx) * hover) * w;
        let oy = (ry + (dy - ry) * hover) * h;
        let s = R::new(body.x + (w - sw) / 2.0 + ox, body.bottom() - h * 0.26 - sh + oy, sw, sh);
        ui.shadow(s, radius * 0.55, 6.0, 0.0, 2.0, [0.0, 0.0, 0.0, 0.16]);
        ui.rect(s, radius * 0.55, [1.0; 4]);
        let c = &p.cards[i];
        if let Some(img) = c.thumb.as_ref() {
            let inner = R::new(s.x + 3.0, s.y + 3.0, s.w - 6.0, s.h - 6.0);
            let k = (inner.w / img.w as f32).max(inner.h / img.h as f32);
            let (iw, ih) = (img.w as f32 * k, img.h as f32 * k);
            ui.clip(inner);
            ui.bitmap(
                &format!("memory|{}|{}x{}", c.id, img.w, img.h),
                img.w,
                img.h,
                &img.rgba,
                R::new(inner.x + (inner.w - iw) / 2.0, inner.y + (inner.h - ih) / 2.0, iw, ih),
            );
            ui.unclip();
        }
    }
    let front = R::new(body.x, body.y + tab, w, h - tab);
    ui.rect(front, radius, top);
    ui.rect(R::new(front.x + radius, front.y, front.w - radius * 2.0, 1.0), 0.0, [1.0, 1.0, 1.0, 0.35]);
    let cx = r.x + r.w / 2.0;
    let b = ui.baseline(Face::Sans600, 13.0, r.bottom() + 16.0);
    let nw = ui.measure(&f.name, Face::Sans600, 13.0).min(r.w + 16.0);
    ui.truncate(cx - nw / 2.0, b, &f.name, Face::Sans600, 13.0, ink(), r.w + 16.0);
    let meta = if f.count == 1 { "1 memory".to_string() } else { format!("{} memories", f.count) };
    let mw = ui.measure(&meta, Face::Sans400, 11.0);
    let b = ui.baseline(Face::Sans400, 11.0, r.bottom() + 33.0);
    ui.text(cx - mw / 2.0, b, &meta, Face::Sans400, 11.0, muted());
}

pub fn palette(ui: &mut Ui, area: R, p: &mut Palette) -> Option<PaletteOut> {
    let t = ui.appear(id(&["palette"]), 220.0, 0.0, Ease::Snap);
    let w = 560.0_f32.min(area.w - 48.0);
    let rows = p.visible();
    let folders = p.folders();
    let row_h = 72.0;
    let (cell_w, cell_h) = (120.0, 150.0);
    let per_row = (((w - 32.0) / cell_w).floor() as usize).max(1);
    let grid_rows = folders.len().div_ceil(per_row);
    let crumb_h = if p.dir.is_empty() || !p.query.trim().is_empty() { 0.0 } else { 34.0 };
    let grid_h = grid_rows as f32 * cell_h;
    let max_rows = if grid_rows > 0 { 4 } else { 6 };
    let list_h = if p.asking.is_some() {
        300.0
    } else if rows.is_empty() && grid_rows > 0 {
        crumb_h + grid_h + 8.0
    } else {
        crumb_h + grid_h + (rows.len().clamp(1, max_rows) as f32) * row_h + 8.0
    };
    let h = (56.0 + list_h + 36.0).min(area.h - 48.0);
    let r = R::new(area.x + (area.w - w) / 2.0, area.y + (area.h - h) * 0.38 + (1.0 - t) * 8.0, w, h);
    ui.push_alpha(t);
    ui.rect(area, 0.0, [0.0, 0.0, 0.0, 0.28]);
    ui.region(area, id(&["palette", "scrim"]));
    ui.shadow(r, 18.0, 40.0, 0.0, 16.0, [0.0, 0.0, 0.0, 0.5]);
    ui.rect(r, 18.0, surface());
    ui.border(r, 18.0, edge(), 1.0);
    ui.region(r, id(&["palette", "panel"]));
    let mut out = None;
    let field = R::new(r.x + 46.0, r.y + 11.0, r.w - 62.0, 34.0);
    if let Some(i) = p.asking {
        if let Some(path) = p.cards[i].full.take()
            && let Some(img) = std::fs::read(path).ok().and_then(|b| koon_core::Image::decode_png(&b).ok())
        {
            p.cards[i].thumb = Some(img.fit(1400));
        }
        let c = &p.cards[i];
        let key = id(&["palette", "ask"]);
        ui.s.focus = Some(key);
        ui.icon("bookmark-plus", r.x + 16.0, r.y + 19.0, 18.0, accent());
        let res = koon_ui::widgets::text_field(ui, key, field, &mut p.text, &format!("¿Qué querés hacer con «{}»?", c.name), Face::Sans400, 15.0, ink());
        ui.rect(R::new(r.x, r.y + 56.0, r.w, 1.0), 0.0, edge());
        let big = R::new(r.x + 16.0, r.y + 68.0, r.w - 32.0, list_h - 56.0);
        thumb(ui, c, big);
        let base = ui.baseline(Face::Sans500, 14.0, r.y + 56.0 + list_h - 22.0);
        let end = ui.text(r.x + 16.0, base, &c.name, Face::Sans500, 14.0, ink());
        if !c.note.is_empty() {
            ui.truncate(end + 10.0, base, &c.note, Face::Sans400, 13.0, muted(), r.right() - end - 30.0);
        }
        if res.submitted.is_some() && !p.text.trim().is_empty() {
            out = Some(PaletteOut::Use(c.id.clone(), p.text.trim().to_string()));
        }
        if ui.key(koon_ui::Key::Escape).is_some() {
            p.asking = None;
            p.text.clear();
        }
    } else {
        let key = id(&["palette", "search"]);
        ui.s.focus = Some(key);
        ui.icon("search", r.x + 16.0, r.y + 19.0, 18.0, muted());
        let before = p.query.clone();
        let hint = if p.dir.is_empty() {
            "Buscar memories".to_string()
        } else {
            format!("Buscar en {}", p.dir.rsplit('/').next().unwrap_or(&p.dir))
        };
        let res = koon_ui::widgets::text_field(ui, key, field, &mut p.query, &hint, Face::Sans400, 15.0, ink());
        if p.query != before {
            p.sel = 0;
        }
        ui.rect(R::new(r.x, r.y + 56.0, r.w, 1.0), 0.0, edge());
        let total = folders.len() + rows.len();
        let browsing = p.query.is_empty();
        if (browsing && ui.key(koon_ui::Key::Right).is_some()) || ui.key(koon_ui::Key::Down).is_some() {
            p.sel = (p.sel + 1).min(total.saturating_sub(1));
        }
        if (browsing && ui.key(koon_ui::Key::Left).is_some()) || ui.key(koon_ui::Key::Up).is_some() {
            p.sel = p.sel.saturating_sub(1);
        }
        let mut y = r.y + 60.0;
        ui.clip(R::new(r.x, r.y + 57.0, r.w, r.h - 57.0 - 36.0));
        if crumb_h > 0.0 {
            let ck = id(&["palette", "crumb"]);
            let cr = R::new(r.x + 10.0, y + 2.0, r.w - 20.0, 28.0);
            ui.region(cr, ck);
            let hot = ui.hovered(ck);
            if hot {
                ui.rect(cr, 8.0, [1.0, 1.0, 1.0, 0.06]);
                ui.cursor(koon_ui::Cursor::Pointer);
            }
            let b = ui.baseline(Face::Sans500, 13.0, cr.y + cr.h / 2.0);
            let mut x = ui.text(cr.x + 10.0, b, "‹  Memories", Face::Sans500, 13.0, muted());
            let parts: Vec<&str> = p.dir.split('/').collect();
            for (k, part) in parts.iter().enumerate() {
                x = ui.text(x + 6.0, b, "/", Face::Sans400, 13.0, muted());
                x = ui.text(x + 6.0, b, part, Face::Sans500, 13.0, if k + 1 == parts.len() { ink() } else { muted() });
            }
            if ui.clicked(ck) {
                p.up();
            }
            y += crumb_h;
        }
        let mut open_dir = None;
        for (k, f) in folders.iter().enumerate() {
            let (col, row) = (k % per_row, k / per_row);
            let used = per_row.min(folders.len());
            let x0 = r.x + (r.w - used as f32 * cell_w) / 2.0;
            let cell = R::new(x0 + col as f32 * cell_w, y + row as f32 * cell_h, cell_w, cell_h);
            let fk = id(&["palette", "folder", &f.path]);
            ui.region(cell, fk);
            let hot = ui.hovered(fk);
            if hot {
                ui.cursor(koon_ui::Cursor::Pointer);
            }
            if k == p.sel {
                ui.rect(R::new(cell.x + 4.0, cell.y + 4.0, cell.w - 8.0, cell.h - 8.0), 14.0, [1.0, 1.0, 1.0, 0.06]);
            }
            let hover = ui.anim(id(&["palette", "fan", &f.path]), if hot || k == p.sel { 1.0 } else { 0.0 }, 280.0, Ease::Bezier(0.24, 1.0, 0.4, 1.0));
            folder(ui, p, f, R::new(cell.x + 16.0, cell.y + 38.0, cell.w - 32.0, 68.0), hover, k);
            if ui.clicked(fk) {
                open_dir = Some(f.path.clone());
            }
        }
        y += grid_h;
        let fit = (((r.bottom() - 36.0 - y) / row_h).floor() as usize).max(1);
        let sel_row = p.sel.checked_sub(folders.len());
        let first = sel_row.map_or(0, |s| s.saturating_sub(fit - 1));
        if rows.is_empty() && folders.is_empty() {
            let msg = if p.cards.is_empty() {
                "Todavía no guardaste memories. Usá «Guardar memory» en el dock."
            } else if p.query.trim().is_empty() {
                "Esta carpeta está vacía."
            } else {
                "Ninguna memory coincide."
            };
            let base = ui.baseline(Face::Sans400, 13.0, y + 30.0);
            ui.text(r.x + 18.0, base, msg, Face::Sans400, 13.0, muted());
        }
        for (k, &i) in rows.iter().enumerate().skip(first).take(fit) {
            let c = &p.cards[i];
            let row = R::new(r.x + 8.0, y + (k - first) as f32 * row_h, r.w - 16.0, row_h - 4.0);
            let rk = id(&["palette", "row", &c.id]);
            ui.region(row, rk);
            if ui.hovered(rk) {
                ui.cursor(koon_ui::Cursor::Pointer);
            }
            if Some(k) == sel_row {
                ui.rect(row, 12.0, [1.0, 1.0, 1.0, 0.08]);
            }
            thumb(ui, c, R::new(row.x + 8.0, row.y + 6.0, 88.0, row.h - 12.0));
            let b1 = ui.baseline(Face::Sans500, 14.0, row.y + row.h / 2.0 - 9.0);
            ui.truncate(row.x + 110.0, b1, &c.name, Face::Sans500, 14.0, ink(), row.w - 170.0);
            let b2 = ui.baseline(Face::Mono500, 11.0, row.y + row.h / 2.0 + 11.0);
            let place = if !p.query.trim().is_empty() && !c.path.is_empty() {
                format!("{}/ · ", c.path)
            } else {
                String::new()
            };
            let sub = if c.note.is_empty() {
                format!("{place}@{}", c.id)
            } else {
                format!("{place}@{} · {}", c.id, c.note)
            };
            ui.truncate(row.x + 110.0, b2, &sub, Face::Mono500, 11.0, muted(), row.w - 170.0);
            let dk = id(&["palette", "delete", &c.id]);
            let doomed = p.doomed.as_deref() == Some(c.id.as_str());
            let cell = if doomed {
                R::new(row.right() - 92.0, row.y + (row.h - 28.0) / 2.0, 82.0, 28.0)
            } else {
                R::new(row.right() - 40.0, row.y + (row.h - 28.0) / 2.0, 28.0, 28.0)
            };
            let over = ui.hovered(dk);
            if doomed {
                ui.rect(cell, 8.0, hex("#E5484D", 1.0));
                ui.icon("trash-2", cell.x + 8.0, cell.y + 6.0, 16.0, [1.0; 4]);
                let b = ui.baseline(Face::Sans500, 12.0, cell.y + cell.h / 2.0);
                ui.text(cell.x + 29.0, b, "¿Borrar?", Face::Sans500, 12.0, [1.0; 4]);
            } else if ui.hovered(rk) || over {
                if over {
                    ui.rect(cell, 8.0, [1.0, 1.0, 1.0, 0.08]);
                }
                ui.icon("trash-2", cell.x + 6.0, cell.y + 6.0, 16.0, if over { hex("#E5484D", 1.0) } else { muted() });
            }
            ui.region(cell, dk);
            if ui.hovered(rk) || over || doomed {
                if over {
                    ui.cursor(koon_ui::Cursor::Pointer);
                }
                if ui.clicked(dk) {
                    if doomed {
                        out = Some(PaletteOut::Delete(c.id.clone()));
                    } else {
                        p.doomed = Some(c.id.clone());
                    }
                    continue;
                }
            }
            if ui.clicked(rk) && !over {
                p.doomed = None;
                p.sel = folders.len() + k;
                p.asking = Some(i);
            }
        }
        ui.unclip();
        if res.submitted.is_some() {
            match (folders.get(p.sel), sel_row.and_then(|s| rows.get(s))) {
                (Some(f), _) => open_dir = Some(f.path.clone()),
                (None, Some(&i)) => p.asking = Some(i),
                _ => {}
            }
        }
        if let Some(d) = open_dir {
            p.dir = d;
            p.query.clear();
            p.sel = 0;
            p.doomed = None;
        } else if ui.key(koon_ui::Key::Escape).is_some() {
            if p.dir.is_empty() || !p.query.trim().is_empty() {
                out = Some(PaletteOut::Close);
            } else {
                p.up();
            }
        } else if ui.key(koon_ui::Key::Backspace).is_some() && before.is_empty() && !p.dir.is_empty() {
            p.up();
        }
    }
    let hint = ui.baseline(Face::Sans400, 11.0, r.bottom() - 18.0);
    let parts: &[(&str, &str)] = if p.asking.is_some() {
        &[("↵", "enviar al agente"), ("esc", "volver")]
    } else if p.dir.is_empty() {
        &[("↵", "abrir"), ("esc", "cerrar")]
    } else {
        &[("↵", "abrir"), ("esc", "volver")]
    };
    let mut x = r.x + 18.0;
    for (k, label) in parts {
        x = ui.text(x, hint, k, Face::Mono500, 11.0, ink());
        x = ui.text(x + 5.0, hint, label, Face::Sans400, 11.0, muted()) + 14.0;
    }
    ui.pop_alpha();
    if out.is_none() && ui.clicked(id(&["palette", "scrim"])) && !ui.hovered(id(&["palette", "panel"])) {
        out = Some(PaletteOut::Close);
    }
    out
}

pub fn toast(ui: &mut Ui, area: R, text: &str, age: f64) {
    let t = (age / 220.0).min(1.0).min(((3200.0 - age) / 400.0).max(0.0)) as f32;
    if t <= 0.0 {
        return;
    }
    let w = ui.measure(text, Face::Sans500, 13.0) + 48.0;
    let r = R::new(area.x + (area.w - w) / 2.0, area.bottom() - 84.0 + (1.0 - t) * 8.0, w, 36.0);
    ui.push_alpha(t);
    ui.shadow(r, 18.0, 20.0, 0.0, 8.0, [0.0, 0.0, 0.0, 0.4]);
    ui.rect(r, 18.0, surface());
    ui.icon("check", r.x + 14.0, r.y + 10.0, 16.0, accent());
    let base = ui.baseline(Face::Sans500, 13.0, r.y + 18.0);
    ui.text(r.x + 36.0, base, text, Face::Sans500, 13.0, ink());
    ui.pop_alpha();
}
