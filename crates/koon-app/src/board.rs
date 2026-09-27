use crate::look::{self, Act, BubbleOut, DockLook, Mic, Palette, PaletteOut};
use crate::voice::{Phase, Take};
use koon_core::{Author, Entry, Kind, Mark, Say, Session, Status};
use koon_ui::{Cursor, Key, Ui, id};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Comment,
    Draw,
    Area,
    Memory,
    Voice,
}

impl Tool {
    pub fn act(self) -> Act {
        match self {
            Tool::Comment => Act::Comment,
            Tool::Draw => Act::Draw,
            Tool::Area => Act::Area,
            Tool::Memory => Act::SaveMemory,
            Tool::Voice => Act::Voice,
        }
    }
}

pub struct Shown {
    pub session: Session,
}

struct Editing {
    session: String,
    id: u32,
    text: String,
    original: String,
    fresh: bool,
    reply: bool,
    memory: bool,
    voiced: bool,
}

#[derive(Debug, PartialEq)]
pub enum Out {
    Act(Act),
    Close,
    Saved(String),
    Dirty(String),
    Memory(String, [f32; 4]),
    UseMemory(String, String),
    Detect([f32; 2], bool),
    Listen(bool),
    DeleteMemory(String),
}

pub struct Snap {
    sid: String,
    mid: u32,
    boxes: Vec<[f32; 4]>,
    idx: usize,
    wheel: f32,
}

pub struct Board {
    pub areas: Vec<koon_ui::R>,
    pub shown: Vec<Shown>,
    pub draft: Option<Session>,
    pub armed: Option<Tool>,
    drawing: Option<u32>,
    editing: Option<Editing>,
    pub palette: Option<Palette>,
    pub notice: Option<(String, f64)>,
    snap: Option<Snap>,
    pub veil: bool,
    pub voice: Option<Take>,
    caret_end: bool,
}

const DISCARDED: &str = "descartado por el usuario";

fn key(sid: &str, mid: u32) -> u64 {
    id(&["mark", sid, &mid.to_string()])
}

impl Board {
    pub fn new() -> Board {
        Board {
            areas: Vec::new(),
            shown: Vec::new(),
            draft: None,
            armed: None,
            drawing: None,
            editing: None,
            palette: None,
            notice: None,
            snap: None,
            veil: false,
            voice: None,
            caret_end: false,
        }
    }

    fn session_mut(&mut self, sid: &str) -> Option<&mut Session> {
        if let Some(d) = self.draft.as_mut().filter(|d| d.id == sid) {
            return Some(d);
        }
        self.shown.iter_mut().map(|s| &mut s.session).find(|s| s.id == sid)
    }

    fn is_draft(&self, sid: &str) -> bool {
        self.draft.as_ref().is_some_and(|d| d.id == sid)
    }

    pub fn capturing(&self) -> bool {
        self.armed.is_some() || self.drawing.is_some()
    }

    pub fn editing(&self) -> bool {
        self.editing.is_some()
    }

    pub fn marks(&self) -> impl Iterator<Item = &Mark> {
        self.shown.iter().flat_map(|s| s.session.marks.iter()).chain(self.draft.iter().flat_map(|d| d.marks.iter()))
    }

    pub fn commit(&mut self) -> Vec<Out> {
        self.drawing = None;
        self.armed = None;
        let mut out = Vec::new();
        if self.editing.is_some() {
            self.finish(BubbleOut::Save, &mut out);
        }
        out
    }

    pub fn retire(&mut self) -> Option<Session> {
        let d = self.draft.take()?;
        if d.marks.iter().any(|m| m.status != Status::Draft) {
            self.shown.push(Shown { session: d.clone() });
            None
        } else {
            Some(d)
        }
    }

    pub fn clear(&mut self) -> Vec<String> {
        self.editing = None;
        self.drawing = None;
        self.armed = None;
        let mut dirty = Vec::new();
        let sessions = self.draft.iter_mut().chain(self.shown.iter_mut().map(|s| &mut s.session));
        for s in sessions {
            let mut touched = false;
            s.marks.retain(|m| m.status != Status::Draft);
            for m in s.marks.iter_mut().filter(|m| m.status.open()) {
                m.status = Status::Resolved;
                m.note = Some(DISCARDED.into());
                touched = true;
            }
            if touched {
                dirty.push(s.id.clone());
            }
        }
        dirty
    }

    pub fn snapped(&mut self, boxes: Vec<[f32; 4]>, memory: bool, now: f64) {
        let Some(first) = boxes.first().copied() else {
            self.notice = Some(("No encontré un elemento ahí. Arrastrá para marcarlo a mano.".into(), now));
            return;
        };
        if self.editing.is_some() {
            return;
        }
        let Some(d) = self.draft.as_mut() else { return };
        let mid = d.next_id();
        let sid = d.id.clone();
        let mut m = Mark::new(mid, Kind::Area, [first[0], first[1]]);
        m.points.push([first[0] + first[2], first[1] + first[3]]);
        d.marks.push(m);
        if boxes.len() > 1 {
            self.notice = Some(("Rueda del mouse: agrandar o achicar la selección".into(), now));
        }
        self.snap = Some(Snap {
            sid: sid.clone(),
            mid,
            boxes,
            idx: 0,
            wheel: 0.0,
        });
        self.open_as(&sid, mid, true, memory);
    }

    fn finish(&mut self, how: BubbleOut, out: &mut Vec<Out>) {
        let next = how == BubbleOut::SaveNext;
        let how = if next { BubbleOut::Save } else { how };
        let tool = self.editing.as_ref().and_then(|e| {
            let kind = self
                .draft
                .iter()
                .chain(self.shown.iter().map(|s| &s.session))
                .find(|s| s.id == e.session)
                .and_then(|s| s.mark(e.id))
                .map(|m| m.kind);
            match (e.memory, e.reply, kind) {
                (true, _, _) => Some(Tool::Memory),
                (_, _, Some(Kind::Pin)) if e.voiced => Some(Tool::Voice),
                (_, true, _) => None,
                (_, _, Some(Kind::Pin)) => Some(Tool::Comment),
                (_, _, Some(Kind::Stroke)) => Some(Tool::Draw),
                (_, _, Some(Kind::Area)) => Some(Tool::Area),
                _ => None,
            }
        });
        self.finish_as(how, out);
        if next && tool.is_some() {
            self.armed = tool;
        }
    }

    fn finish_as(&mut self, how: BubbleOut, out: &mut Vec<Out>) {
        self.snap = None;
        self.voice = None;
        let Some(e) = self.editing.take() else { return };
        let draft = self.is_draft(&e.session);
        let Some(s) = self.session_mut(&e.session) else { return };
        let Some(pos) = s.marks.iter().position(|m| m.id == e.id) else { return };
        let text = e.text.trim().to_string();
        if e.memory {
            let bounds = s.marks[pos].bounds();
            s.marks.remove(pos);
            if how == BubbleOut::Save && !text.is_empty() {
                out.push(Out::Memory(text, bounds));
            }
            return;
        }
        if e.reply {
            match how {
                BubbleOut::Save if !text.is_empty() => {
                    let m = &mut s.marks[pos];
                    m.thread.push(Entry::new(Author::User, Say::Reply, text));
                    m.status = Status::Pending;
                    m.note = None;
                    out.push(if draft { Out::Saved(e.session) } else { Out::Dirty(e.session) });
                }
                BubbleOut::Delete => {
                    s.marks[pos].status = Status::Resolved;
                    s.marks[pos].note = Some(DISCARDED.into());
                    out.push(if draft { Out::Saved(e.session) } else { Out::Dirty(e.session) });
                }
                _ => {}
            }
            return;
        }
        match how {
            BubbleOut::Save | BubbleOut::SaveNext | BubbleOut::Mic => {
                let m = &mut s.marks[pos];
                let changed = m.text != text || m.status == Status::Draft;
                m.text = text;
                if !m.meaningful() {
                    s.marks.remove(pos);
                } else if changed {
                    s.marks[pos].status = Status::Pending;
                    s.marks[pos].note = None;
                    out.push(if draft { Out::Saved(e.session) } else { Out::Dirty(e.session) });
                }
            }
            BubbleOut::Cancel => {
                let m = &mut s.marks[pos];
                m.text = e.original;
                if e.fresh && m.kind == Kind::Pin {
                    s.marks.remove(pos);
                } else if e.fresh && m.meaningful() {
                    m.status = Status::Pending;
                    out.push(Out::Saved(e.session));
                }
            }
            BubbleOut::Delete => {
                if s.marks[pos].status == Status::Draft {
                    s.marks.remove(pos);
                } else {
                    s.marks[pos].status = Status::Resolved;
                    s.marks[pos].note = Some(DISCARDED.into());
                    out.push(if draft { Out::Saved(e.session) } else { Out::Dirty(e.session) });
                }
            }
        }
    }

    fn open(&mut self, sid: &str, mid: u32, fresh: bool) {
        self.open_as(sid, mid, fresh, false);
    }

    fn open_as(&mut self, sid: &str, mid: u32, fresh: bool, memory: bool) {
        let mark = self.session_mut(sid).and_then(|s| s.mark(mid)).cloned();
        let reply = !fresh && mark.as_ref().is_some_and(|m| m.status != Status::Draft);
        let text = if reply { String::new() } else { mark.map(|m| m.text).unwrap_or_default() };
        self.editing = Some(Editing {
            session: sid.to_string(),
            id: mid,
            original: text.clone(),
            text,
            fresh,
            reply,
            memory,
            voiced: false,
        });
    }

    pub fn heard(&mut self, take: u64, text: &str) -> bool {
        if self.voice.as_ref().is_none_or(|v| v.id != take) {
            return false;
        }
        self.voice = None;
        let Some(e) = self.editing.as_mut() else { return false };
        if !text.is_empty() {
            if !e.text.is_empty() && !e.text.ends_with(' ') {
                e.text.push(' ');
            }
            e.text.push_str(text);
            self.caret_end = true;
        }
        true
    }

    fn mic(&self) -> Mic {
        match self.voice.as_ref() {
            None => Mic::Off,
            Some(v) => match v.phase() {
                Phase::Listening if v.listening() => Mic::Listening(v.level()),
                Phase::Downloading => Mic::Busy("Descargando el modelo de voz (solo la primera vez)…"),
                _ => Mic::Busy("Transcribiendo…"),
            },
        }
    }

    pub fn blur(&mut self) -> Vec<Out> {
        let mut out = Vec::new();
        if self.editing.is_some() && self.drawing.is_none() {
            self.finish(BubbleOut::Save, &mut out);
        }
        out
    }

    pub fn draw(&mut self, ui: &mut Ui, dock: Option<DockLook>, review: bool) -> Vec<Out> {
        let mut out = Vec::new();
        if self.veil {
            return out;
        }
        let editing = self.editing.as_ref().map(|e| (e.session.clone(), e.id));
        let mut open: Option<(String, u32)> = None;
        let mut gone = Vec::new();
        let interactive = self.drawing.is_none();
        let sessions: Vec<(usize, &Session)> = self.shown.iter().map(|s| &s.session).chain(self.draft.iter()).enumerate().collect();
        let shown_len = self.shown.len();
        for (i, s) in sessions {
            let mut alive = false;
            for m in &s.marks {
                let done = m.status == Status::Resolved && (m.done().is_some() || m.note.as_deref().is_some_and(|n| n != DISCARDED));
                let focus = editing.as_ref().is_some_and(|(sid, mid)| *sid == s.id && *mid == m.id);
                let reopen = id(&["reopen", &s.id, &m.id.to_string()]);
                let holding = done && (focus || ui.hovered(reopen));
                if holding {
                    ui.set_anim(key(&s.id, m.id), 1.0);
                }
                let a = ui.anim_delay(
                    key(&s.id, m.id),
                    if m.status == Status::Resolved && !holding { 0.0 } else { 1.0 },
                    450.0,
                    if done { 2600.0 } else { 0.0 },
                    koon_ui::Ease::Css,
                );
                if a > 0.01 {
                    alive = true;
                }
                let k = key(&s.id, m.id);
                let reveal = ui.anim(id(&["label", &k.to_string()]), if review || ui.hovered(k) { 1.0 } else { 0.0 }, 160.0, koon_ui::Ease::Css);
                look::mark(ui, m, a, focus, false);
                let asked = m.status != Status::Resolved && m.asking().is_some();
                if asked && !focus {
                    ui.push_alpha(a);
                    look::ask(ui, m, m.asking().unwrap_or(""));
                    ui.pop_alpha();
                } else if done && a > 0.01 && !focus {
                    ui.push_alpha(a);
                    let hot = ui.hovered(reopen);
                    let r = look::done(ui, m, m.done().or(m.note.as_deref()).unwrap_or(""), hot);
                    ui.pop_alpha();
                    if interactive {
                        ui.region(r, reopen);
                        if hot {
                            ui.cursor(Cursor::Pointer);
                            ui.s.animating = true;
                        }
                        if ui.clicked(reopen) {
                            open = Some((s.id.clone(), m.id));
                        }
                    }
                } else if !focus && self.drawing != Some(m.id) && reveal > 0.01 {
                    ui.push_alpha(a * reveal);
                    look::label(ui, m);
                    ui.pop_alpha();
                }
                if interactive && m.status != Status::Resolved && m.meaningful() {
                    ui.region(look::badge_rect(m), k);
                    if ui.hovered(k) {
                        ui.cursor(Cursor::Pointer);
                    }
                    if ui.clicked(k) {
                        open = Some((s.id.clone(), m.id));
                    }
                }
            }
            if i < shown_len && !alive && s.marks.iter().all(|m| m.status == Status::Resolved) {
                gone.push(i);
            }
        }
        for i in gone.into_iter().rev() {
            self.shown.remove(i);
        }
        if let Some(sn) = self.snap.as_mut() {
            let b = sn.boxes[sn.idx];
            let r = koon_ui::R::new(b[0], b[1], b[2], b[3]);
            ui.region(r, id(&["snap", &sn.mid.to_string()]));
            sn.wheel += ui.input.wheel.1;
            let step = if sn.wheel >= 1.0 {
                1
            } else if sn.wheel <= -1.0 {
                -1
            } else {
                0
            };
            if step != 0 {
                sn.wheel = 0.0;
                let next = (sn.idx as i64 + step).clamp(0, sn.boxes.len() as i64 - 1) as usize;
                if next != sn.idx {
                    sn.idx = next;
                    let b = sn.boxes[next];
                    let (sid, mid) = (sn.sid.clone(), sn.mid);
                    if let Some(m) = self.session_mut(&sid).and_then(|s| s.mark_mut(mid)) {
                        m.points = vec![[b[0], b[1]], [b[0] + b[2], b[1] + b[3]]];
                    }
                }
            }
        }
        let had_bubble = self.editing.is_some();
        if let Some((sid, mid)) = editing {
            let mark = self.session_mut(&sid).and_then(|s| s.mark(mid)).cloned();
            let anchor = mark.as_ref().map(look::badge_rect);
            match anchor {
                Some(anchor) if self.editing.as_ref().is_some_and(|e| e.reply) => {
                    let full = koon_ui::R::new(0.0, 0.0, ui.width, ui.height);
                    let bounds = self.areas.iter().copied().find(|r| r.contains((anchor.x + anchor.w / 2.0, anchor.y + anchor.h / 2.0))).unwrap_or(full);
                    let e = self.editing.as_mut().expect("editing");
                    if let Some(how) = look::thread(ui, key(&sid, mid), anchor, bounds, mark.as_ref().expect("mark"), &mut e.text) {
                        self.finish(how, &mut out);
                    }
                }
                Some(anchor) => {
                    let placeholder = if self.editing.as_ref().is_some_and(|e| e.memory) {
                        "Nombrá esta memory (ej. botones/tarjeta linear)"
                    } else if self.editing.as_ref().is_some_and(|e| e.fresh) {
                        "¿Qué hay que cambiar acá?"
                    } else {
                        "Editar comentario"
                    };
                    let full = koon_ui::R::new(0.0, 0.0, ui.width, ui.height);
                    let bounds = self.areas.iter().copied().find(|r| r.contains((anchor.x + anchor.w / 2.0, anchor.y + anchor.h / 2.0))).unwrap_or(full);
                    let mic = self.mic();
                    let tint = mark.as_ref().map_or(look::accent(), look::mark_color);
                    if std::mem::take(&mut self.caret_end) {
                        let n = self.editing.as_ref().map_or(0, |e| e.text.chars().count());
                        let f = ui.s.fields.entry(look::field_key(key(&sid, mid))).or_default();
                        f.caret = n;
                        f.anchor = n;
                    }
                    let e = self.editing.as_mut().expect("editing");
                    match look::bubble(ui, key(&sid, mid), anchor, bounds, &mut e.text, placeholder, mic, tint) {
                        Some(BubbleOut::Mic) => out.push(Out::Listen(mic == Mic::Off)),
                        Some(BubbleOut::Save | BubbleOut::SaveNext) if matches!(mic, Mic::Listening(_)) => out.push(Out::Listen(false)),
                        Some(BubbleOut::Save | BubbleOut::SaveNext) if matches!(mic, Mic::Busy(_)) => {}
                        Some(how) => self.finish(how, &mut out),
                        None => {}
                    }
                }
                None => self.editing = None,
            }
        }
        if let Some(look) = dock
            && let Some(act) = look::dock(ui, &look)
        {
            let pick = |t: Tool, armed: Option<Tool>| if armed == Some(t) { None } else { Some(t) };
            match act {
                Act::Comment => self.armed = pick(Tool::Comment, self.armed),
                Act::Draw => self.armed = pick(Tool::Draw, self.armed),
                Act::Area => self.armed = pick(Tool::Area, self.armed),
                Act::SaveMemory => self.armed = pick(Tool::Memory, self.armed),
                Act::Voice if self.editing.as_ref().is_some_and(|e| !e.reply) => out.push(Out::Listen(self.voice.is_none())),
                Act::Voice => self.armed = pick(Tool::Voice, self.armed),
                other => out.push(Out::Act(other)),
            }
            if self.armed.is_some() && self.editing.is_some() {
                self.finish(BubbleOut::Save, &mut out);
            }
        }
        if let Some((sid, mid)) = open {
            if self.editing.as_ref().is_some_and(|e| e.session != sid || e.id != mid) {
                self.finish(BubbleOut::Save, &mut out);
            }
            if self.editing.is_none() {
                self.armed = None;
                self.open(&sid, mid, false);
            }
        }
        let mouse = [ui.input.mouse.0, ui.input.mouse.1];
        let free = !ui.over_ui();
        if let Some(tool) = self.armed
            && free
            && self.drawing.is_none()
        {
            ui.cursor(if matches!(tool, Tool::Comment | Tool::Voice) { Cursor::Pointer } else { Cursor::Crosshair });
            if ui.input.pressed
                && let Some(d) = self.draft.as_mut()
            {
                let mid = d.next_id();
                let sid = d.id.clone();
                match tool {
                    Tool::Comment | Tool::Voice => {
                        d.marks.push(Mark::new(mid, Kind::Pin, mouse));
                        self.armed = None;
                        self.open(&sid, mid, true);
                        if tool == Tool::Voice {
                            if let Some(e) = self.editing.as_mut() {
                                e.voiced = true;
                            }
                            out.push(Out::Listen(true));
                        }
                    }
                    Tool::Draw => {
                        d.marks.push(Mark::new(mid, Kind::Stroke, mouse));
                        self.drawing = Some(mid);
                    }
                    Tool::Area | Tool::Memory => {
                        let mut m = Mark::new(mid, Kind::Area, mouse);
                        m.points.push(mouse);
                        d.marks.push(m);
                        self.drawing = Some(mid);
                    }
                }
            }
        }
        if let Some(mid) = self.drawing {
            let saving = self.armed == Some(Tool::Memory);
            ui.cursor(Cursor::Crosshair);
            let released = ui.input.released || !ui.input.down;
            if let Some(d) = self.draft.as_mut()
                && let Some(m) = d.mark_mut(mid)
            {
                match m.kind {
                    Kind::Stroke => {
                        let last = *m.points.last().expect("points");
                        if (last[0] - mouse[0]).hypot(last[1] - mouse[1]) >= 2.0 {
                            m.points.push(mouse);
                        }
                    }
                    Kind::Area => m.points[1] = mouse,
                    Kind::Pin => {}
                }
                ui.s.animating = true;
                if released {
                    self.drawing = None;
                    self.armed = None;
                    let tiny = m.kind == Kind::Area && (m.points[1][0] - m.points[0][0]).abs() < 6.0 && (m.points[1][1] - m.points[0][1]).abs() < 6.0;
                    if tiny {
                        let at = m.points[0];
                        d.marks.retain(|m| m.id != mid);
                        out.push(Out::Detect(at, saving));
                    } else if m.meaningful() {
                        let sid = d.id.clone();
                        self.open_as(&sid, mid, true, saving);
                    } else {
                        d.marks.retain(|m| m.id != mid);
                    }
                }
            } else {
                self.drawing = None;
            }
        }
        if !had_bubble && self.drawing.is_none() && self.palette.is_none() {
            let plain = |m: koon_ui::Mods| !(m.ctrl || m.meta || m.alt);
            if ui.key(Key::Escape).is_some() {
                if self.armed.is_some() {
                    self.armed = None;
                } else {
                    out.push(Out::Close);
                }
            }
            if ui.key(Key::Char('c')).is_some_and(plain) {
                self.armed = Some(Tool::Comment);
            }
            if ui.key(Key::Char('d')).is_some_and(plain) {
                self.armed = Some(Tool::Draw);
            }
            if ui.key(Key::Char('a')).is_some_and(plain) {
                self.armed = Some(Tool::Area);
            }
            if ui.key(Key::Char('m')).is_some_and(plain) {
                self.armed = Some(Tool::Memory);
            }
            if ui.key(Key::Char('v')).is_some_and(plain) {
                self.armed = Some(Tool::Voice);
            }
        }
        let full = koon_ui::R::new(0.0, 0.0, ui.width, ui.height);
        if let Some((text, at)) = self.notice.clone() {
            let area = self.areas.first().copied().unwrap_or(full);
            let age = ui.now - at;
            look::toast(ui, area, &text, age);
            if age > 3600.0 {
                self.notice = None;
            } else {
                ui.s.animating = true;
            }
        }
        if let Some(p) = self.palette.as_mut() {
            let area = self.areas.first().copied().unwrap_or(full);
            match look::palette(ui, area, p) {
                Some(PaletteOut::Close) => self.palette = None,
                Some(PaletteOut::Delete(id)) => {
                    p.cards.retain(|c| c.id != id);
                    p.doomed = None;
                    p.sel = p.sel.min(p.cards.len().saturating_sub(1));
                    out.push(Out::DeleteMemory(id));
                }
                Some(PaletteOut::Use(id, text)) => {
                    self.palette = None;
                    out.push(Out::UseMemory(id, text));
                }
                None => {}
            }
        }
        out
    }
}
