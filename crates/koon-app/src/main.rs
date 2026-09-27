mod board;
mod capture;
mod gfx;
mod ipc;
mod look;
#[cfg(target_os = "linux")]
mod mutter;
mod preview;
#[cfg(target_os = "linux")]
mod probe;
#[cfg(target_os = "linux")]
mod shape;
mod system;
mod voice;

use board::{Board, Out, Shown};
use gfx::{Gpu, View, mods, ui_key};
use koon_core::{Author, Image, Kind, Mark, Session, Status, Store};
use koon_ui::Css;
use look::{Act, DockLook, PillLook};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::Key as WKey;
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

#[cfg(test)]
mod tests {
    use super::*;
    use koon_core::{Kind, Mark};

    fn session(status: Status) -> Session {
        let mut m = Mark::new(1, Kind::Pin, [0.0, 0.0]);
        m.text = "hello".into();
        m.status = status;
        Session {
            id: "s".into(),
            created: 0,
            size: [1.0, 1.0],
            scale: 1.0,
            monitor: None,
            monitors: Vec::new(),
            screen: None,
            annotated: None,
            preview: None,
            marks: vec![m],
        }
    }

    #[test]
    fn statuses_only_move_forward() {
        let mut mine = session(Status::Taken);
        merge(&session(Status::Resolved), &mut mine);
        assert_eq!(mine.marks[0].status, Status::Resolved);
        let mut mine = session(Status::Pending);
        merge(&session(Status::Taken), &mut mine);
        assert_eq!(mine.marks[0].status, Status::Taken);
        let mut mine = session(Status::Resolved);
        merge(&session(Status::Pending), &mut mine);
        assert_eq!(mine.marks[0].status, Status::Resolved);
        let mut asked = session(Status::Taken);
        asked.marks[0].thread.push(koon_core::Entry {
            author: Author::Agent,
            say: koon_core::Say::Question,
            text: "green or gold?".into(),
            at: 10,
        });
        let mut mine = session(Status::Taken);
        merge(&asked, &mut mine);
        assert_eq!(mine.marks[0].asking(), Some("green or gold?"));
        mine.marks[0].thread.push(koon_core::Entry {
            author: Author::User,
            say: koon_core::Say::Reply,
            text: "gold".into(),
            at: 20,
        });
        merge(&asked, &mut mine);
        assert_eq!(mine.marks[0].status, Status::Pending);
        assert_eq!(mine.marks[0].thread.len(), 2);
    }
}

#[derive(Debug)]
enum Ev {
    Toggle,
    Memories,
    MemorySaved(usize, Result<String, String>),
    Detected(usize, Vec<[f32; 4]>, bool),
    Show,
    Hide,
    Reload,
    Tick,
    Quit,
    Snapped(usize, String, Option<Image>),
    Saved(String),
    Heard(u64, Result<String, String>),
    Redraw,
    DragTick,
}

type Rect = (f32, f32, f32, f32);

static DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct Mon {
    name: Option<String>,
    rect: (i32, i32, u32, u32),
}

struct Screen {
    view: View,
    board: Board,
    origin: (i32, i32),
    monitors: Vec<Mon>,
    input: Option<Vec<Rect>>,
}

struct App {
    proxy: EventLoopProxy<Ev>,
    gpu: Option<Gpu>,
    pill: Option<View>,
    screens: Vec<Screen>,
    start: Instant,
    store: Arc<Store>,
    open: bool,
    typing: bool,
    closing: Option<Instant>,
    armed: Option<board::Tool>,
    hidden: bool,
    pill_hit: bool,
    pill_since: Option<Instant>,
    press: Option<(f32, f32)>,
    press_global: Option<((i32, i32), (i32, i32))>,
    dragging: Option<((i32, i32), (i32, i32))>,
    grab: Option<((i32, i32), (i32, i32))>,
    modified: HashMap<String, SystemTime>,
    #[cfg(target_os = "linux")]
    shaper: Option<shape::Shaper>,
    css: Css,
    _hotkey: Option<global_hotkey::GlobalHotKeyManager>,
}

fn attrs(name: &str, dock: bool) -> WindowAttributes {
    let a = Window::default_attributes()
        .with_title("koon")
        .with_decorations(false)
        .with_transparent(true)
        .with_window_level(WindowLevel::AlwaysOnTop);
    #[cfg(target_os = "macos")]
    let a = {
        use winit::platform::macos::WindowAttributesExtMacOS;
        a.with_decorations(true)
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_titlebar_buttons_hidden(true)
            .with_fullsize_content_view(true)
            .with_has_shadow(false)
    };
    #[cfg(target_os = "linux")]
    let a = {
        use winit::platform::x11::{WindowAttributesExtX11, WindowType};
        let a = winit::platform::wayland::WindowAttributesExtWayland::with_name(a, system::APP_ID, name);
        a.with_x11_window_type(vec![if dock { WindowType::Dock } else { WindowType::Utility }])
    };
    #[cfg(target_os = "windows")]
    let a = winit::platform::windows::WindowAttributesExtWindows::with_skip_taskbar(a, true);
    let _ = (name, dock);
    a
}

fn rank(s: Status) -> u8 {
    match s {
        Status::Draft => 0,
        Status::Pending => 1,
        Status::Taken => 2,
        Status::Resolved => 3,
    }
}

fn merge(disk: &Session, mine: &mut Session) {
    for m in &mut mine.marks {
        let Some(d) = disk.mark(m.id) else { continue };
        if m.status == Status::Draft {
            continue;
        }
        let before = m.thread.len();
        m.merge_thread(&d.thread);
        let agent_new = m.thread.len() > before;
        let unsent = m.last().is_some_and(|e| e.author == Author::User && !d.thread.contains(e));
        if unsent {
            m.status = Status::Pending;
            continue;
        }
        if d.text == m.text && (rank(d.status) > rank(m.status) || agent_new || d.status == m.status && d.note != m.note) {
            m.status = d.status;
            m.note = d.note.clone();
        }
    }
}

fn save(store: &Store, d: &mut Session, image: Option<Image>) -> std::io::Result<()> {
    if let Some(img) = image {
        store.write(&d.id, "annotated.png", &img.encode_png()?)?;
        d.annotated = Some("annotated.png".into());
        if img.w.max(img.h) > 1600 {
            store.write(&d.id, "preview.png", &img.fit(1600).encode_png()?)?;
            d.preview = Some("preview.png".into());
        }
        for m in d.marks.iter().filter(|m| m.status.open()) {
            let b = m.bounds();
            let s = d.scale;
            let (cx, cy) = ((b[0] + b[2] / 2.0) * s, (b[1] + b[3] / 2.0) * s);
            let (w, h) = ((b[2] + 320.0).max(560.0) * s, (b[3] + 240.0).max(360.0) * s);
            let crop = img.crop((cx - w / 2.0) as i64, (cy - h / 2.0) as i64, w as i64, h as i64);
            if crop.w > 0 && crop.h > 0 {
                store.write(&d.id, &format!("crop-{}.png", m.id), &crop.fit(1200).encode_png()?)?;
            }
        }
    }
    if let Ok(disk) = store.load(&d.id) {
        merge(&disk, d);
    }
    store.save(d)
}

impl App {
    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }

    fn redraw_all(&self) {
        for v in self.pill.iter().chain(self.screens.iter().map(|s| &s.view)) {
            v.window.request_redraw();
        }
    }

    fn open_screens(&mut self, el: &ActiveEventLoop) {
        let Some(gpu) = self.gpu.as_ref() else { return };
        let mut handles: Vec<MonitorHandle> = el.available_monitors().collect();
        if handles.is_empty() {
            handles.extend(el.primary_monitor());
        }
        #[cfg(target_os = "linux")]
        let known = mutter::connectors();
        let count = handles.len();
        let monitors: Vec<Mon> = handles
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let (p, z) = (m.position(), m.size());
                #[cfg(target_os = "linux")]
                let name = mutter::pick(&known, (p.x, p.y), i, count).or_else(|| m.name());
                #[cfg(not(target_os = "linux"))]
                let name = {
                    let _ = (i, count);
                    m.name()
                };
                Mon {
                    name,
                    rect: (p.x, p.y, z.width, z.height),
                }
            })
            .collect();
        let x0 = monitors.iter().map(|m| m.rect.0).min().unwrap_or(0);
        let y0 = monitors.iter().map(|m| m.rect.1).min().unwrap_or(0);
        let x1 = monitors.iter().map(|m| m.rect.0 + m.rect.2 as i32).max().unwrap_or(800);
        let y1 = monitors.iter().map(|m| m.rect.1 + m.rect.3 as i32).max().unwrap_or(600);
        let a = attrs("koon-overlay", true)
            .with_inner_size(winit::dpi::PhysicalSize::new((x1 - x0) as u32, (y1 - y0) as u32))
            .with_position(winit::dpi::PhysicalPosition::new(x0, y0))
            .with_resizable(false);
        let window = Arc::new(el.create_window(a).expect("overlay window"));
        let mut board = Board::new();
        let scale = window.scale_factor() as f32;
        board.areas = monitors
            .iter()
            .map(|m| koon_ui::R::new((m.rect.0 - x0) as f32 / scale, (m.rect.1 - y0) as f32 / scale, m.rect.2 as f32 / scale, m.rect.3 as f32 / scale))
            .collect();
        let desktop = [(x1 - x0) as f32 / scale, (y1 - y0) as f32 / scale];
        for session in self.store.list().into_iter().filter(|s| !s.monitors.is_empty() && s.open().next().is_some()) {
            if session.size != desktop {
                eprintln!("koon: session {} belongs to a different monitor layout; showing it anyway", session.id);
            }
            if let Some(m) = self.store.modified(&session.id) {
                self.modified.insert(session.id.clone(), m);
            }
            board.shown.push(Shown { session });
        }
        self.screens.push(Screen {
            view: gpu.view(window, None),
            board,
            origin: (x0, y0),
            monitors,
            input: None,
        });
        self.shape(0, Vec::new());
    }

    fn shape(&mut self, i: usize, rects: Vec<Rect>) {
        let Some(s) = self.screens.get_mut(i) else { return };
        if s.input.as_ref() == Some(&rects) {
            return;
        }
        #[cfg(target_os = "linux")]
        let done = self.shaper.as_ref().is_some_and(|sh| sh.input(&s.view.window, &rects));
        #[cfg(not(target_os = "linux"))]
        let done = false;
        if !done {
            let _ = s.view.window.set_cursor_hittest(!rects.is_empty());
        }
        s.input = Some(rects);
    }

    fn pill_screen(&self) -> usize {
        0
    }

    fn toggle(&mut self) {
        if self.open { self.close() } else { self.show_dock() }
    }

    fn show_dock(&mut self) {
        self.hidden = false;
        self.open = true;
        koon_core::ipc::set_state(true, self.typing);
        self.closing = None;
        for s in &mut self.screens {
            if s.board.draft.is_none() {
                let size = s.view.logical();
                match self.store.create([size.0, size.1], s.view.scale()) {
                    Ok(mut d) => {
                        let names: Vec<String> = s.monitors.iter().filter_map(|m| m.name.clone()).collect();
                        d.monitor = Some(names.join(" + "));
                        d.monitors = s.monitors.iter().zip(&s.board.areas).map(|(m, r)| (m.name.clone().unwrap_or_default(), [r.x, r.y, r.w, r.h])).collect();
                        s.board.draft = Some(d);
                    }
                    Err(e) => eprintln!("koon: cannot create session: {e}"),
                }
            }
        }
        let i = self.pill_screen();
        if let Some(s) = self.screens.get(i) {
            s.view.window.focus_window();
        }
        self.redraw_all();
    }

    fn close(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        self.typing = false;
        koon_core::ipc::set_state(false, false);
        self.armed = None;
        self.closing = Some(Instant::now());
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(320));
            let _ = proxy.send_event(Ev::Redraw);
        });
        let mut outs = Vec::new();
        for s in &mut self.screens {
            outs.extend(s.board.commit());
        }
        self.apply_all(outs);
        for s in &mut self.screens {
            if let Some(empty) = s.board.retire() {
                let _ = std::fs::remove_dir_all(self.store.dir(&empty.id));
            }
        }
        self.redraw_all();
    }

    fn memories(&mut self) {
        let i = self.pill_screen();
        if self.screens.get(i).is_some_and(|s| s.board.palette.is_some()) {
            self.screens[i].board.palette = None;
            self.redraw_all();
            return;
        }
        if !self.open {
            self.show_dock();
        }
        let store = self.store.clone();
        let cards = store
            .memories()
            .into_iter()
            .map(|m| look::Card {
                thumb: std::fs::read(store.memory_file(&m.id, "thumb.png")).ok().and_then(|b| Image::decode_png(&b).ok()),
                full: Some(store.memory_file(&m.id, "image.png")),
                id: m.id,
                name: m.name,
                note: m.note,
                path: m.path,
            })
            .collect();
        if let Some(s) = self.screens.get_mut(i) {
            s.board.armed = None;
            s.board.palette = Some(look::Palette::new(cards));
            s.view.window.focus_window();
        }
        self.armed = None;
        self.redraw_all();
    }

    fn save_memory(&mut self, i: usize, text: String, b: [f32; 4]) {
        let Some(s) = self.screens.get(i) else { return };
        let (name, note) = match text.split_once(':') {
            Some((n, rest)) if !n.trim().is_empty() => (n.trim().to_string(), rest.trim().to_string()),
            _ => (text.clone(), String::new()),
        };
        let mons: Vec<(Option<String>, (i32, i32, u32, u32))> = s.monitors.iter().map(|m| (m.name.clone(), m.rect)).collect();
        let monitor = s
            .board
            .areas
            .iter()
            .position(|r| r.contains((b[0] + b[2] / 2.0, b[1] + b[3] / 2.0)))
            .and_then(|k| s.monitors.get(k))
            .and_then(|m| m.name.clone());
        let origin = s.origin;
        let size = (s.view.config.width, s.view.config.height);
        let scale = s.view.scale();
        let store = self.store.clone();
        let proxy = self.proxy.clone();
        if let Some(s) = self.screens.get_mut(i) {
            s.board.veil = true;
        }
        self.redraw_all();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(320));
            let r = capture::desktop(&mons, origin, size).and_then(|img| {
                let img = img.resize(size.0, size.1);
                let crop = img.crop((b[0] * scale) as i64, (b[1] * scale) as i64, (b[2] * scale).round() as i64, (b[3] * scale).round() as i64);
                if crop.w < 4 || crop.h < 4 {
                    return Err("el área es demasiado chica".into());
                }
                store.save_memory(&name, &note, &crop, monitor).map(|m| m.id).map_err(|e| e.to_string())
            });
            let _ = proxy.send_event(Ev::MemorySaved(i, r));
        });
    }

    fn detect(&mut self, i: usize, at: [f32; 2], memory: bool) {
        let Some(s) = self.screens.get(i) else { return };
        let mons: Vec<(Option<String>, (i32, i32, u32, u32))> = s.monitors.iter().map(|m| (m.name.clone(), m.rect)).collect();
        let origin = s.origin;
        let size = (s.view.config.width, s.view.config.height);
        let scale = s.view.scale();
        let proxy = self.proxy.clone();
        if let Some(s) = self.screens.get_mut(i) {
            s.board.veil = true;
        }
        self.redraw_all();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(140));
            let boxes = capture::desktop(&mons, origin, size)
                .map(|img| {
                    let img = img.resize(size.0, size.1);
                    koon_core::detect::boxes(&img, ((at[0] * scale) as u32, (at[1] * scale) as u32))
                        .into_iter()
                        .map(|b| [b[0] as f32 / scale, b[1] as f32 / scale, b[2] as f32 / scale, b[3] as f32 / scale])
                        .collect()
                })
                .unwrap_or_else(|e| {
                    eprintln!("koon: capture: {e}");
                    Vec::new()
                });
            if std::env::var_os("KOON_DEBUG").is_some() {
                eprintln!("detect at {at:?} scale {scale} size {size:?}: {boxes:?}");
            }
            let _ = proxy.send_event(Ev::Detected(i, boxes, memory));
        });
    }

    fn use_memory(&mut self, i: usize, id: String, text: String) {
        let Some(s) = self.screens.get_mut(i) else { return };
        let area = s.board.areas.first().copied().unwrap_or_else(|| {
            let (w, h) = s.view.logical();
            koon_ui::R::new(0.0, 0.0, w, h)
        });
        let Some(d) = s.board.draft.as_mut() else { return };
        let mid = d.next_id();
        let mut m = Mark::new(mid, Kind::Pin, [area.x + area.w / 2.0, area.y + area.h * 0.3]);
        m.text = text;
        m.memory = Some(id);
        m.status = Status::Pending;
        d.marks.push(m);
        let sid = d.id.clone();
        self.snap(i, sid);
        self.redraw_all();
    }

    fn hide(&mut self) {
        self.close();
        self.hidden = true;
        self.redraw_all();
    }

    fn screen_of(&self, sid: &str) -> Option<usize> {
        self.screens
            .iter()
            .position(|s| s.board.draft.as_ref().is_some_and(|d| d.id == sid) || s.board.shown.iter().any(|x| x.session.id == sid))
    }

    fn session(&self, i: usize, sid: &str) -> Option<Session> {
        let b = &self.screens.get(i)?.board;
        b.draft.iter().chain(b.shown.iter().map(|s| &s.session)).find(|s| s.id == sid).cloned()
    }

    fn snap(&mut self, i: usize, sid: String) {
        let Some(s) = self.screens.get(i) else { return };
        let mons: Vec<(Option<String>, (i32, i32, u32, u32))> = s.monitors.iter().map(|m| (m.name.clone(), m.rect)).collect();
        let origin = s.origin;
        let size = (s.view.config.width, s.view.config.height);
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(260));
            let img = capture::desktop(&mons, origin, size).map_err(|e| eprintln!("koon: capture: {e}")).ok();
            let _ = proxy.send_event(Ev::Snapped(i, sid, img));
        });
    }

    fn snapped(&mut self, i: usize, sid: String, img: Option<Image>) {
        let Some(mut session) = self.session(i, &sid) else { return };
        let (w, h) = (self.screens[i].view.config.width, self.screens[i].view.config.height);
        let img = img.map(|im| im.resize(w, h));
        session.marks.retain(|m| m.status != Status::Draft && m.meaningful());
        let store = self.store.clone();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || match save(&store, &mut session, img) {
            Ok(()) => {
                let _ = proxy.send_event(Ev::Saved(session.id.clone()));
            }
            Err(e) => eprintln!("koon: could not save {}: {e}", session.id),
        });
    }

    fn apply(&mut self, i: usize, outs: Vec<Out>) {
        for o in outs {
            match o {
                Out::Close => self.close(),
                Out::Saved(sid) | Out::Dirty(sid) => {
                    let at = self.screen_of(&sid).unwrap_or(i);
                    self.snap(at, sid);
                }
                Out::Act(Act::Clear) => {
                    let dirty: Vec<(usize, String)> = self.screens.iter_mut().enumerate().flat_map(|(k, s)| s.board.clear().into_iter().map(move |sid| (k, sid))).collect();
                    for (k, sid) in dirty {
                        self.snap(k, sid);
                    }
                }
                Out::Act(Act::Grab) => self.grab_dock(),
                Out::Act(Act::Memories) => self.memories(),
                Out::Memory(name, bounds) => self.save_memory(i, name, bounds),
                Out::UseMemory(id, text) => self.use_memory(i, id, text),
                Out::DeleteMemory(id) => {
                    let now = self.now();
                    let msg = match self.store.delete_memory(&id) {
                        Ok(()) => format!("Memory borrada · @{id}"),
                        Err(e) => format!("No se pudo borrar: {e}"),
                    };
                    if let Some(s) = self.screens.get_mut(i) {
                        s.board.notice = Some((msg, now));
                    }
                }
                Out::Detect(at, memory) => self.detect(i, at, memory),
                Out::Listen(on) => self.listen(i, on),
                Out::Act(_) => {}
            }
        }
    }

    fn listen(&mut self, i: usize, on: bool) {
        let Some(s) = self.screens.get_mut(i) else { return };
        if !on {
            if let Some(v) = s.board.voice.as_ref() {
                v.stop();
            }
        } else if s.board.editing() {
            let proxy = self.proxy.clone();
            s.board.voice = Some(voice::Take::start(self.store.root().to_path_buf(), move |id, r| {
                let _ = proxy.send_event(Ev::Heard(id, r));
            }));
        }
        self.redraw_all();
    }

    fn heard(&mut self, take: u64, r: Result<String, String>) {
        let now = self.now();
        let text = r.as_deref().unwrap_or("");
        for s in &mut self.screens {
            if s.board.heard(take, text) {
                s.board.notice = match &r {
                    Ok(t) if t.is_empty() => Some(("No escuché nada. Acercate al micrófono y probá de nuevo.".into(), now)),
                    Err(e) => Some((format!("Voz: {e}"), now)),
                    Ok(_) => None,
                };
            }
        }
        if let Err(e) = &r {
            eprintln!("koon: voice: {e}");
        }
        self.redraw_all();
    }

    fn apply_all(&mut self, outs: Vec<Out>) {
        self.apply(0, outs);
    }

    fn reload(&mut self) {
        let mut changed = false;
        let ids: Vec<String> = self
            .screens
            .iter()
            .flat_map(|s| s.board.draft.iter().map(|d| d.id.clone()).chain(s.board.shown.iter().map(|x| x.session.id.clone())))
            .collect();
        for sid in ids {
            let Some(m) = self.store.modified(&sid) else { continue };
            if self.modified.get(&sid) == Some(&m) {
                continue;
            }
            self.modified.insert(sid.clone(), m);
            let Ok(disk) = self.store.load(&sid) else { continue };
            for s in &mut self.screens {
                let b = &mut s.board;
                for session in b.draft.iter_mut().chain(b.shown.iter_mut().map(|x: &mut Shown| &mut x.session)).filter(|x| x.id == sid) {
                    merge(&disk, session);
                    changed = true;
                }
            }
        }
        if changed {
            self.redraw_all();
        }
    }

    fn dock_at(&self, i: usize) -> ((f32, f32), bool, koon_ui::R) {
        let Some(s) = self.screens.get(i) else { return ((0.0, 0.0), false, koon_ui::R::default()) };
        let o = &s.view;
        let sc = o.scale() as f64;
        let placed = match (self.pill.as_ref().map(|p| p.window.outer_position()), o.window.outer_position()) {
            (Some(Ok(p)), Ok(q)) => Some((((p.x - q.x) as f64 / sc) as f32 + look::MARGIN, ((p.y - q.y) as f64 / sc) as f32 + look::MARGIN)),
            _ => None,
        };
        let area = placed
            .and_then(|(x, y)| s.board.areas.iter().copied().find(|r| r.contains((x + look::PILL.0 / 2.0, y + look::PILL.1 / 2.0))))
            .or(s.board.areas.first().copied())
            .unwrap_or_else(|| {
                let (w, h) = o.logical();
                koon_ui::R::new(0.0, 0.0, w, h)
            });
        let (x, y) = placed.unwrap_or((area.right() - look::DOCK_W - 24.0, area.y + (area.h - look::PILL.1) / 2.0));
        let up = y + look::dock_height(false) > area.bottom() - 12.0 && y + look::PILL.1 - look::dock_height(true) >= area.y + 12.0;
        ((x, y), up, area)
    }

    fn remember_pill(&self) {
        if let Some(Ok(at)) = self.pill.as_ref().map(|p| p.window.outer_position()) {
            let mut prefs = self.store.prefs();
            prefs.pill = Some((at.x, at.y));
            if let Err(e) = self.store.save_prefs(&prefs) {
                eprintln!("koon: could not save preferences: {e}");
            }
        }
    }

    fn drag_ticks(proxy: EventLoopProxy<Ev>) {
        DRAGGING.store(true, std::sync::atomic::Ordering::Relaxed);
        std::thread::spawn(move || {
            while proxy.send_event(Ev::DragTick).is_ok() {
                std::thread::sleep(Duration::from_millis(8));
                if !DRAGGING.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    fn grab_dock(&mut self) {
        #[cfg(target_os = "linux")]
        let g = self.shaper.as_ref().and_then(|sh| sh.pointer());
        #[cfg(not(target_os = "linux"))]
        let g: Option<(i32, i32)> = None;
        let at = self.pill.as_ref().and_then(|p| p.window.outer_position().ok()).map(|p| (p.x, p.y));
        match g.zip(at) {
            Some(start) => {
                self.grab = Some(start);
                App::drag_ticks(self.proxy.clone());
            }
            None => self.close(),
        }
    }

    fn drag_tick(&mut self) {
        if let Some((g0, w0)) = self.grab {
            #[cfg(target_os = "linux")]
            let state = self.shaper.as_ref().and_then(|sh| sh.pointer_state());
            #[cfg(not(target_os = "linux"))]
            let state: Option<((i32, i32), bool)> = None;
            let Some((g, down)) = state else { return };
            if !down {
                self.grab = None;
                DRAGGING.store(false, std::sync::atomic::Ordering::Relaxed);
                self.close();
                return;
            }
            if (g.0 - g0.0).abs().max((g.1 - g0.1).abs()) <= 4 {
                return;
            }
            self.grab = None;
            self.dragging = Some((g0, w0));
        }
        let Some((g0, w0)) = self.dragging else {
            DRAGGING.store(false, std::sync::atomic::Ordering::Relaxed);
            return;
        };
        #[cfg(target_os = "linux")]
        let state = self.shaper.as_ref().and_then(|sh| sh.pointer_state());
        #[cfg(not(target_os = "linux"))]
        let state: Option<((i32, i32), bool)> = None;
        let Some((g, down)) = state else { return };
        if std::env::var_os("KOON_DEBUG").is_some() {
            eprintln!("drag g0={g0:?} w0={w0:?} g={g:?} down={down}");
        }
        if let Some(p) = &self.pill {
            p.window.set_outer_position(winit::dpi::PhysicalPosition::new(w0.0 + g.0 - g0.0, w0.1 + g.1 - g0.1));
        }
        if self.open {
            for s in &self.screens {
                s.view.window.request_redraw();
            }
        }
        if !down {
            self.dragging = None;
            self.press = None;
            DRAGGING.store(false, std::sync::atomic::Ordering::Relaxed);
            self.remember_pill();
        }
    }

    fn draw_pill(&mut self) {
        let now = self.now();
        let closing = self.closing.is_some_and(|t| t.elapsed() < Duration::from_millis(320));
        let hidden = self.hidden || self.open || closing;
        let marks = || self.screens.iter().flat_map(|s| s.board.marks());
        let open = marks().filter(|m| m.status.open()).count();
        let working = marks().any(|m| m.status == Status::Taken);
        let handoff = self.closing.is_some() && !self.hidden;
        let shadow = if handoff {
            self.pill_since.map_or(0.0, |t| t.elapsed().as_secs_f32() / 0.15).min(1.0)
        } else {
            1.0
        };
        let (Some(gpu), Some(v)) = (self.gpu.as_ref(), self.pill.as_mut()) else { return };
        v.frame(gpu, &self.css, now, |ui| {
            look::pill(
                ui,
                &PillLook {
                    level: 0.0,
                    listening: false,
                    busy: false,
                    hidden,
                    open,
                    working,
                    shadow,
                },
            );
        });
        if self.pill_hit == hidden {
            self.pill_hit = !hidden;
            self.pill_since = (!hidden).then(Instant::now);
            let _ = v.window.set_cursor_hittest(!hidden);
            for s in &self.screens {
                s.view.window.request_redraw();
            }
        }
    }

    fn draw_overlay(&mut self, i: usize) {
        let now = self.now();
        let docked = i == self.pill_screen();
        let (at, up, area) = self.dock_at(i);
        let open = self.open;
        let fade = self.pill_since.map_or(0.0, |t| t.elapsed().as_secs_f32() / 0.15);
        let linger = (self.closing.is_some() && !self.hidden && fade < 1.0).then_some(1.0 - fade);
        let Some(gpu) = self.gpu.as_ref() else { return };
        let Some(s) = self.screens.get_mut(i) else { return };
        let was_editing = s.board.editing();
        let armed = self.armed;
        s.board.armed = armed;
        let board = &mut s.board;
        let mut outs = Vec::new();
        s.view.frame(gpu, &self.css, now, |ui| {
            let dock = docked.then(|| DockLook {
                at,
                up,
                area,
                open,
                linger,
                active: board.armed.map(|t| t.act()),
            });
            outs = board.draw(ui, dock, open);
        });
        let (w, h) = s.view.logical();
        let rects: Vec<Rect> = if s.board.capturing() {
            vec![(0.0, 0.0, w, h)]
        } else {
            s.view.state.regions().iter().map(|(r, _)| (r.x, r.y, r.w, r.h)).collect()
        };
        if !was_editing && s.board.editing() {
            if std::env::var_os("KOON_DEBUG").is_some() {
                eprintln!("bubble opened on overlay {i}, focusing");
            }
            s.view.window.focus_window();
        }
        let now_armed = s.board.armed;
        let typing = self.screens.iter().any(|s| s.board.editing());
        if typing != self.typing {
            self.typing = typing;
            koon_core::ipc::set_state(self.open, typing);
        }
        self.shape(i, rects);
        if now_armed != armed {
            self.armed = now_armed;
            self.redraw_all();
        }
        if !outs.is_empty() {
            self.apply(i, outs);
            self.redraw_all();
        }
    }

    fn command(&mut self, ev: Ev) {
        match ev {
            Ev::Toggle => self.toggle(),
            Ev::Memories => self.memories(),
            Ev::Detected(i, boxes, memory) => {
                let now = self.now();
                if let Some(s) = self.screens.get_mut(i) {
                    s.board.veil = false;
                    s.board.snapped(boxes, memory, now);
                    s.view.window.focus_window();
                }
                self.redraw_all();
            }
            Ev::MemorySaved(i, r) => {
                let now = self.now();
                if let Some(s) = self.screens.get_mut(i) {
                    s.board.veil = false;
                    s.board.notice = Some((
                        match r {
                            Ok(id) => format!("Memory guardada · @{id}"),
                            Err(e) => format!("No se pudo guardar: {e}"),
                        },
                        now,
                    ));
                }
                self.redraw_all();
            }
            Ev::Show => {
                self.hidden = false;
                self.redraw_all();
            }
            Ev::Hide => self.hide(),
            Ev::Reload | Ev::Tick => self.reload(),
            Ev::Quit => {}
            Ev::Snapped(i, sid, img) => self.snapped(i, sid, img),
            Ev::Redraw => self.redraw_all(),
            Ev::Heard(take, r) => self.heard(take, r),
            Ev::DragTick => self.drag_tick(),
            Ev::Saved(sid) => {
                if let Some(m) = self.store.modified(&sid) {
                    self.modified.insert(sid, m);
                }
                self.redraw_all();
            }
        }
    }
}

impl ApplicationHandler<Ev> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.pill.is_some() {
            return;
        }
        let size = LogicalSize::new((look::PILL.0 + look::MARGIN * 2.0) as f64, (look::PILL.1 + look::MARGIN * 2.0) as f64);
        let mut a = attrs("koon", true).with_inner_size(size).with_resizable(false);
        let saved = self.store.prefs().pill.filter(|&(x, y)| {
            el.available_monitors().any(|m| {
                let (p, z) = (m.position(), m.size());
                x >= p.x - 20 && y >= p.y - 20 && x + 60 <= p.x + z.width as i32 && y + 60 <= p.y + z.height as i32
            })
        });
        if let Some((x, y)) = saved {
            a = a.with_position(winit::dpi::PhysicalPosition::new(x, y));
        } else if let Some(m) = el.primary_monitor() {
            let s = m.scale_factor();
            let (p, ms) = (m.position().to_logical::<f64>(s), m.size().to_logical::<f64>(s));
            a = a.with_position(LogicalPosition::new(p.x + ms.width - size.width - 24.0, p.y + ms.height / 2.0 - size.height / 2.0));
        }
        let window = Arc::new(el.create_window(a).expect("pill window"));
        let (gpu, surface) = Gpu::new(window.clone());
        self.pill = Some(gpu.view(window, Some(surface)));
        self.gpu = Some(gpu);
        self.open_screens(el);
        self.redraw_all();
    }

    fn user_event(&mut self, el: &ActiveEventLoop, ev: Ev) {
        if matches!(ev, Ev::Quit) {
            el.exit();
            return;
        }
        self.command(ev);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, wid: WindowId, event: WindowEvent) {
        let is_pill = self.pill.as_ref().is_some_and(|v| v.window.id() == wid);
        let overlay = self.screens.iter().position(|s| s.view.window.id() == wid);
        if !is_pill && overlay.is_none() {
            return;
        }
        match &event {
            WindowEvent::RedrawRequested => {
                match overlay {
                    Some(i) => self.draw_overlay(i),
                    None => self.draw_pill(),
                }
                return;
            }
            WindowEvent::CloseRequested => {
                el.exit();
                return;
            }
            WindowEvent::Focused(f)
                if std::env::var_os("KOON_DEBUG").is_some() && {
                    eprintln!("focus {f} overlay {overlay:?} pill {is_pill}");
                    false
                } => {}
            WindowEvent::Focused(false) => {
                if let Some(i) = overlay {
                    let outs = self.screens[i].board.blur();
                    self.apply(i, outs);
                    self.redraw_all();
                }
                return;
            }
            _ => {}
        }
        let device = self.gpu.as_ref().map(|g| g.device.clone());
        let v = match overlay {
            Some(i) => self.screens.get_mut(i).map(|s| &mut s.view),
            None => self.pill.as_mut(),
        };
        let Some(v) = v else { return };
        match event {
            WindowEvent::Resized(size) => {
                if let Some(d) = &device {
                    v.resize(d, size.width, size.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => v.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let s = v.window.scale_factor();
                v.mouse = ((position.x / s) as f32, (position.y / s) as f32);
                if is_pill {
                    #[cfg(target_os = "linux")]
                    let global = self.shaper.as_ref().and_then(|sh| sh.pointer());
                    #[cfg(not(target_os = "linux"))]
                    let global: Option<(i32, i32)> = None;
                    if let Some(p) = self.press
                        && (p.0 - v.mouse.0).hypot(p.1 - v.mouse.1) > 4.0
                    {
                        self.press = None;
                        match self.press_global.take().or(global.zip(v.window.outer_position().ok().map(|p| (p.x, p.y)))) {
                            Some((g, at)) => {
                                self.dragging = Some((g, at));
                                App::drag_ticks(self.proxy.clone());
                            }
                            _ => {
                                let _ = v.window.drag_window();
                            }
                        }
                    }
                    if let (Some((g0, w0)), Some(g)) = (self.dragging, global) {
                        v.window.set_outer_position(winit::dpi::PhysicalPosition::new(w0.0 + g.0 - g0.0, w0.1 + g.1 - g0.1));
                    }
                    let _ = s;
                }
            }
            WindowEvent::CursorLeft { .. } if !is_pill => v.mouse = (-100.0, -100.0),
            WindowEvent::MouseWheel { delta, .. } if !is_pill => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => (p.y / 40.0) as f32,
                };
                v.input.wheel.1 += dy;
                v.window.request_redraw();
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                if is_pill {
                    match state {
                        ElementState::Pressed => {
                            self.press = Some(v.mouse);
                            let sf = v.window.scale_factor();
                            let at = v.window.outer_position().ok().map(|p| (p.x, p.y));
                            #[cfg(target_os = "linux")]
                            let live = self.shaper.as_ref().and_then(|sh| sh.pointer());
                            #[cfg(not(target_os = "linux"))]
                            let live: Option<(i32, i32)> = None;
                            let from_event = at.map(|p| (p.0 + (v.mouse.0 as f64 * sf) as i32, p.1 + (v.mouse.1 as f64 * sf) as i32));
                            if let (Some(l), Some(p)) = (live, at) {
                                v.mouse = (((l.0 - p.0) as f64 / sf) as f32, ((l.1 - p.1) as f64 / sf) as f32);
                            }
                            self.press = Some(v.mouse);
                            self.press_global = live.or(from_event).zip(at);
                        }
                        ElementState::Released if self.dragging.is_some() => {
                            self.drag_tick();
                            self.dragging = None;
                            DRAGGING.store(false, std::sync::atomic::Ordering::Relaxed);
                            self.remember_pill();
                            return;
                        }
                        ElementState::Released if self.press.take().is_some() => {
                            self.toggle();
                            return;
                        }
                        _ => {}
                    }
                } else if state == ElementState::Pressed {
                    v.input.pressed = true;
                    v.input.down = true;
                } else {
                    v.input.released = true;
                    v.input.down = false;
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if std::env::var_os("KOON_DEBUG").is_some() {
                    eprintln!("key {:?} -> overlay {:?}", event.logical_key, overlay);
                }
                if v.mods.control_key() && matches!(&event.logical_key, WKey::Character(c) if c.eq_ignore_ascii_case("q")) {
                    el.exit();
                    return;
                }
                if let Some(k) = ui_key(&event.logical_key) {
                    v.input.keys.push((k, mods(v.mods)));
                }
                if let Some(t) = event.text.as_ref()
                    && !v.mods.control_key()
                    && !v.mods.super_key()
                    && !t.chars().any(char::is_control)
                {
                    v.input.text.push_str(t);
                }
            }
            _ => return,
        }
        v.window.request_redraw();
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        for s in &mut self.screens {
            if let Some(d) = s.board.draft.take()
                && d.marks.iter().all(|m| m.status == Status::Draft)
            {
                let _ = std::fs::remove_dir_all(self.store.dir(&d.id));
            }
        }
        ipc::cleanup();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(cmd) = args.first().map(String::as_str) {
        match cmd {
            "toggle" | "memories" | "show" | "hide" | "quit" => match koon_core::ipc::send(cmd) {
                Ok(r) => {
                    println!("{r}");
                    return;
                }
                Err(e) => {
                    eprintln!("koon is not running ({e})");
                    std::process::exit(1);
                }
            },
            #[cfg(target_os = "linux")]
            "--probe" => {
                probe::run();
                return;
            }
            #[cfg(target_os = "linux")]
            "--e2e" => {
                probe::e2e();
                return;
            }
            #[cfg(target_os = "linux")]
            "--mark" => {
                probe::mark(&args[1..]);
                return;
            }
            #[cfg(target_os = "linux")]
            "--scene" => {
                probe::scene();
                return;
            }
            #[cfg(target_os = "linux")]
            "--suite-snap" => {
                probe::suite_snap();
                return;
            }
            #[cfg(target_os = "linux")]
            "--suite" => {
                probe::suite();
                return;
            }
            #[cfg(target_os = "linux")]
            "--stress" => {
                probe::stress(args.get(1).and_then(|v| v.parse().ok()).unwrap_or(2));
                return;
            }
            "--listen" => {
                if let Err(e) = voice::listen(args.get(1).and_then(|v| v.parse().ok()).unwrap_or(5)) {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                return;
            }
            "--record" => {
                let secs = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(8);
                if let Err(e) = voice::record_file(secs, args.get(2).map(String::as_str).unwrap_or("koon-voice.wav")) {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                return;
            }
            "--transcribe" => {
                if let Err(e) = voice::transcribe_file(args.get(1).map(String::as_str).unwrap_or("")) {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                return;
            }
            "--morph" => {
                let path = args.get(1).map(String::as_str).unwrap_or("koon-morph.png");
                if let Err(e) = preview::morph(path) {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                println!("{path}");
                return;
            }
            #[cfg(target_os = "linux")]
            "--detect" => {
                probe::detect_probe(args.get(1).map(String::as_str).unwrap_or("detect.png"));
                return;
            }
            "--palette" => {
                let path = args.get(1).cloned().unwrap_or_else(|| "palette.png".into());
                match preview::palette(&path) {
                    Ok(()) => println!("{path}"),
                    Err(e) => eprintln!("{e}"),
                }
                return;
            }
            "--preview" => {
                let path = args.get(1).map(String::as_str).unwrap_or("koon-preview.png");
                if let Err(e) = preview::render(path) {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                println!("{path}");
                return;
            }
            "--help" | "-h" | "help" => {
                println!(
                    "koon            open the widget\nkoon toggle     open or close the options ({})\nkoon show|hide  show or hide the widget\nkoon quit       quit koon",
                    system::SHORTCUT
                );
                return;
            }
            other => {
                eprintln!("unknown command: {other}");
                std::process::exit(2);
            }
        }
    }
    if koon_core::ipc::send("show").is_ok() {
        println!("koon was already open");
        return;
    }
    system::identity(&args);
    #[cfg(target_os = "linux")]
    let el = {
        let mut b = EventLoop::<Ev>::with_user_event();
        if std::env::var_os("KOON_WAYLAND").is_none() && std::env::var_os("DISPLAY").is_some() {
            use winit::platform::x11::EventLoopBuilderExtX11;
            b.with_x11();
        }
        b.build().expect("event loop")
    };
    #[cfg(not(target_os = "linux"))]
    let el = EventLoop::<Ev>::with_user_event().build().expect("event loop");
    let proxy = el.create_proxy();
    let p = proxy.clone();
    if let Err(e) = ipc::serve(move |cmd| {
        let ev = match cmd {
            "toggle" => Ev::Toggle,
            "memories" => Ev::Memories,
            "show" => Ev::Show,
            "hide" => Ev::Hide,
            "reload" => Ev::Reload,
            "quit" => Ev::Quit,
            _ => return false,
        };
        p.send_event(ev).is_ok()
    }) {
        eprintln!("koon: control socket unavailable: {e}");
    }
    let p = proxy.clone();
    std::thread::spawn(move || {
        while p.send_event(Ev::Tick).is_ok() {
            std::thread::sleep(Duration::from_millis(700));
        }
    });
    let p = proxy.clone();
    let q = proxy.clone();
    let hotkey = system::hotkey(
        move || {
            let _ = p.send_event(Ev::Toggle);
        },
        move || {
            let _ = q.send_event(Ev::Memories);
        },
    );
    match system::gnome_shortcut() {
        Ok(true) => eprintln!("koon: shortcuts {} and {} registered in GNOME", system::SHORTCUT, system::MEMORIES),
        Ok(false) => {}
        Err(e) => eprintln!("koon: could not register the GNOME shortcut: {e}"),
    }
    let mut app = App {
        proxy,
        gpu: None,
        pill: None,
        screens: Vec::new(),
        start: Instant::now(),
        store: Arc::new(Store::open()),
        open: false,
        typing: false,
        closing: None,
        armed: None,
        hidden: false,
        pill_hit: true,
        pill_since: None,
        press: None,
        press_global: None,
        dragging: None,
        grab: None,
        modified: HashMap::new(),
        #[cfg(target_os = "linux")]
        shaper: shape::Shaper::new(),
        css: Css::light(),
        _hotkey: hotkey,
    };
    el.run_app(&mut app).expect("koon");
}
