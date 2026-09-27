use std::time::Instant;
use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, SK};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

fn atom(c: &impl Connection, name: &str) -> u32 {
    c.intern_atom(false, name.as_bytes()).ok().and_then(|r| r.reply().ok()).map(|r| r.atom).unwrap_or(0)
}

pub fn run() {
    let known = crate::mutter::connectors();
    println!("mutter monitors: {:?}", known);
    let names: Vec<String> = known.iter().map(|(_, c)| c.clone()).collect();
    let t = Instant::now();
    match crate::mutter::grab(&names) {
        Ok(imgs) => {
            println!("silent capture: {} ms", t.elapsed().as_millis());
            for (n, img) in names.iter().zip(imgs) {
                match img {
                    Some(img) => {
                        let px = (img.w * img.h) as f32;
                        let black = img.rgba.chunks_exact(4).filter(|p| p[0] < 6 && p[1] < 6 && p[2] < 6).count() as f32;
                        println!("  {n}: {}x{} black {:.1}%", img.w, img.h, black / px * 100.0);
                        if let Some(dir) = std::env::var_os("KOON_PROBE_SAVE") {
                            let _ = std::fs::write(std::path::Path::new(&dir).join(format!("{n}.png")), img.encode_png().unwrap_or_default());
                        }
                    }
                    None => println!("  {n}: no frame"),
                }
            }
        }
        Err(e) => println!("capture: {e}"),
    }
    let Ok((c, screen)) = x11rb::connect(None) else {
        println!("no X11");
        return;
    };
    let root = c.setup().roots[screen].root;
    let (class, state, above, net_type) = (atom(&c, "WM_CLASS"), atom(&c, "_NET_WM_STATE"), atom(&c, "_NET_WM_STATE_ABOVE"), atom(&c, "_NET_WM_WINDOW_TYPE"));
    let Ok(tree) = c.query_tree(root).ok().and_then(|r| r.reply().ok()).ok_or(()) else { return };
    let mut stack = tree.children;
    let mut seen = 0;
    while let Some(w) = stack.pop() {
        if let Ok(t) = c.query_tree(w).ok().and_then(|r| r.reply().ok()).ok_or(()) {
            stack.extend(t.children);
        }
        let cls = c
            .get_property(false, w, class, AtomEnum::STRING, 0, 64)
            .ok()
            .and_then(|r| r.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.value).replace('\0', " "))
            .unwrap_or_default();
        if !cls.contains("dev.koon.Koon") {
            continue;
        }
        seen += 1;
        let states: Vec<u32> = c
            .get_property(false, w, state, AtomEnum::ATOM, 0, 32)
            .ok()
            .and_then(|r| r.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default();
        let kind: Vec<u32> = c
            .get_property(false, w, net_type, AtomEnum::ATOM, 0, 8)
            .ok()
            .and_then(|r| r.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default();
        let geo = c.get_geometry(w).ok().and_then(|r| r.reply().ok()).map(|g| (g.width, g.height, g.depth));
        let input = shape::get_rectangles(&c, w, SK::INPUT)
            .ok()
            .and_then(|r| r.reply().ok())
            .map(|r| r.rectangles.iter().map(|x| (x.x, x.y, x.width, x.height)).collect::<Vec<_>>());
        println!("window {w:#x} {cls:?} geo {geo:?} above {} type {kind:?} input {:?}", states.contains(&above), input);
    }
    println!("koon windows: {seen}");
}

mod e2e {
    use super::atom;
    use koon_core::{Status, Store};
    use std::time::Duration;
    use x11rb::connection::Connection;
    use x11rb::protocol::shape::{self, SK};
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
    use x11rb::protocol::xtest::ConnectionExt as _;
    use x11rb::rust_connection::RustConnection;

    struct X {
        c: RustConnection,
        root: Window,
    }

    fn wait(ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    impl X {
        fn owns(&self, x: i16, y: i16) -> bool {
            self.windows().iter().any(|w| {
                let (ox, oy) = (w.2.0, w.2.1);
                w.3.iter().any(|r| x >= ox + r.0 && x < ox + r.0 + r.2 as i16 && y >= oy + r.1 && y < oy + r.1 + r.3 as i16)
            })
        }
        fn focused_koon(&self) -> bool {
            let Some(f) = self.c.get_input_focus().ok().and_then(|r| r.reply().ok()).map(|r| r.focus) else {
                return false;
            };
            self.windows().iter().any(|w| w.0 == f)
        }
        fn safe_click(&self, x: i16, y: i16) -> Result<(), String> {
            if !self.owns(x, y) {
                return Err(format!("({x},{y}) does not belong to koon; aborting to avoid touching other apps"));
            }
            self.click(x, y);
            Ok(())
        }
        fn shift_key(&self, keysym: u32) {
            if let (Some(sh), Some(k)) = (self.keycode(0xffe1), self.keycode(keysym)) {
                let _ = self.c.xtest_fake_input(2, sh, 0, self.root, 0, 0, 0);
                let _ = self.c.xtest_fake_input(2, k, 0, self.root, 0, 0, 0);
                let _ = self.c.xtest_fake_input(3, k, 0, self.root, 0, 0, 0);
                let _ = self.c.xtest_fake_input(3, sh, 0, self.root, 0, 0, 0);
                let _ = self.c.flush();
                wait(25);
            }
        }
        fn motion(&self, x: i16, y: i16) {
            let _ = self.c.xtest_fake_input(6, 0, 0, self.root, x, y, 0);
            let _ = self.c.flush();
        }
        fn click(&self, x: i16, y: i16) {
            self.motion(x, y);
            wait(60);
            let _ = self.c.xtest_fake_input(4, 1, 0, self.root, 0, 0, 0);
            let _ = self.c.flush();
            wait(40);
            let _ = self.c.xtest_fake_input(5, 1, 0, self.root, 0, 0, 0);
            let _ = self.c.flush();
            wait(120);
        }
        fn drag(&self, from: (i16, i16), to: (i16, i16)) -> Result<(), String> {
            if !self.owns(from.0, from.1) {
                return Err(format!("({},{}) does not belong to koon; aborting", from.0, from.1));
            }
            self.motion(from.0, from.1);
            wait(60);
            let _ = self.c.xtest_fake_input(4, 1, 0, self.root, 0, 0, 0);
            let _ = self.c.flush();
            wait(80);
            let dist = ((to.0 - from.0) as f32).hypot((to.1 - from.1) as f32);
            let steps = ((dist / 20.0).ceil() as i32).max(10);
            for i in 1..=steps {
                let t = i as f32 / steps as f32;
                let x = from.0 as f32 + (to.0 - from.0) as f32 * t;
                let y = from.1 as f32 + (to.1 - from.1) as f32 * t + (t * std::f32::consts::PI).sin() * 30.0;
                self.motion(x as i16, y as i16);
                wait(6);
            }
            let _ = self.c.xtest_fake_input(5, 1, 0, self.root, 0, 0, 0);
            let _ = self.c.flush();
            wait(150);
            Ok(())
        }
        fn escape(&self) {
            self.key(0xff1b);
        }
        fn keycode(&self, keysym: u32) -> Option<u8> {
            self.level(keysym).filter(|l| !l.1).map(|l| l.0)
        }
        fn panel(&self, parent: Window, r: (i16, i16, u16, u16), color: u32) -> Result<Window, String> {
            use x11rb::protocol::xproto::{CreateWindowAux, WindowClass};
            let id = self.c.generate_id().map_err(|e| e.to_string())?;
            let aux = CreateWindowAux::new().background_pixel(color);
            self.c
                .create_window(x11rb::COPY_DEPTH_FROM_PARENT, id, parent, r.0, r.1, r.2, r.3, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
                .map_err(|e| e.to_string())?;
            self.c.map_window(id).map_err(|e| e.to_string())?;
            let _ = self.c.flush();
            Ok(id)
        }
        fn level(&self, keysym: u32) -> Option<(u8, bool)> {
            let setup = self.c.setup();
            let (min, max) = (setup.min_keycode, setup.max_keycode);
            let map = self.c.get_keyboard_mapping(min, max - min + 1).ok()?.reply().ok()?;
            let per = map.keysyms_per_keycode as usize;
            let rows: Vec<&[u32]> = map.keysyms.chunks(per).collect();
            if let Some(i) = rows.iter().position(|k| k.first() == Some(&keysym)) {
                return Some((min + i as u8, false));
            }
            rows.iter().position(|k| k.get(1) == Some(&keysym)).map(|i| (min + i as u8, true))
        }
        fn key(&self, keysym: u32) {
            if let Some(k) = self.keycode(keysym) {
                let _ = self.c.xtest_fake_input(2, k, 0, self.root, 0, 0, 0);
                let _ = self.c.xtest_fake_input(3, k, 0, self.root, 0, 0, 0);
                let _ = self.c.flush();
                wait(25);
            }
        }
        fn typing(&self, text: &str) {
            for ch in text.chars() {
                let sym = if ch.is_ascii_uppercase() { ch.to_ascii_lowercase() as u32 } else { ch as u32 };
                let Some((k, shifted)) = self.level(sym).map(|(k, s)| (k, s || ch.is_ascii_uppercase())) else {
                    continue;
                };
                let shift = if shifted { self.keycode(0xffe1) } else { None };
                if let Some(sh) = shift {
                    let _ = self.c.xtest_fake_input(2, sh, 0, self.root, 0, 0, 0);
                }
                let _ = self.c.xtest_fake_input(2, k, 0, self.root, 0, 0, 0);
                let _ = self.c.xtest_fake_input(3, k, 0, self.root, 0, 0, 0);
                if let Some(sh) = shift {
                    let _ = self.c.xtest_fake_input(3, sh, 0, self.root, 0, 0, 0);
                }
                let _ = self.c.flush();
                wait(25);
            }
        }
        fn windows(&self) -> Vec<(Window, String, (i16, i16, u16, u16), Vec<(i16, i16, u16, u16)>)> {
            let class = atom(&self.c, "WM_CLASS");
            let mut out = Vec::new();
            let mut stack = self.c.query_tree(self.root).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
            while let Some(w) = stack.pop() {
                if let Some(t) = self.c.query_tree(w).ok().and_then(|r| r.reply().ok()) {
                    stack.extend(t.children);
                }
                let cls = self
                    .c
                    .get_property(false, w, class, AtomEnum::STRING, 0, 64)
                    .ok()
                    .and_then(|r| r.reply().ok())
                    .map(|r| String::from_utf8_lossy(&r.value).replace('\0', " "))
                    .unwrap_or_default();
                if !cls.contains("dev.koon.Koon") {
                    continue;
                }
                let Some(g) = self.c.get_geometry(w).ok().and_then(|r| r.reply().ok()) else { continue };
                let Some(t) = self.c.translate_coordinates(w, self.root, 0, 0).ok().and_then(|r| r.reply().ok()) else {
                    continue;
                };
                let input = shape::get_rectangles(&self.c, w, SK::INPUT)
                    .ok()
                    .and_then(|r| r.reply().ok())
                    .map(|r| r.rectangles.iter().map(|x| (x.x, x.y, x.width, x.height)).collect())
                    .unwrap_or_default();
                out.push((w, cls, (t.dst_x, t.dst_y, g.width, g.height), input));
            }
            out
        }
    }

    fn latest(n: usize) -> Vec<koon_core::Session> {
        let all = Store::open().list();
        all.into_iter().rev().take(n).collect()
    }

    fn marks_since(before: u64) -> Vec<(koon_core::Session, koon_core::Mark)> {
        Store::open()
            .list()
            .into_iter()
            .filter(|s| s.created >= before)
            .flat_map(|s| s.marks.clone().into_iter().map(move |m| (s.clone(), m)))
            .collect()
    }

    fn snap_check(x: &X, ov: (i16, i16, u16, u16), start: u64, check: &mut impl FnMut(&str, bool, String)) -> Result<(), String> {
        let open_dock = |x: &X| -> Result<(i16, i16), String> {
            let wins = x.windows();
            if let Some(d) = wins.iter().filter(|w| w.2.2 > 400).find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1))) {
                return Ok(d);
            }
            let p = wins.iter().find(|w| w.2.2 == 76).map(|w| w.2).ok_or("no pill")?;
            x.safe_click(p.0 + 38, p.1 + 64)?;
            wait(700);
            x.windows()
                .iter()
                .filter(|w| w.2.2 > 400)
                .find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1)))
                .ok_or_else(|| "the dock did not open".to_string())
        };
        let item = |d: (i16, i16), i: i16| (d.0 + 26, d.1 + 76 + 40 * i + if i >= 6 { 8 } else { 0 });
        let page = x.panel(x.root, (ov.0 + 420, ov.1 + 300, 420, 260), 0xf5f5f7)?;
        let card = x.panel(page, (40, 40, 340, 180), 0xffffff)?;
        let button = x.panel(card, (60, 110, 150, 44), 0x3b6fe0)?;
        wait(1200);
        let at = x.c.translate_coordinates(button, x.root, 0, 0).ok().and_then(|r| r.reply().ok()).ok_or("test window not mapped")?;
        let bx = (at.dst_x, at.dst_y);
        let dock = open_dock(x)?;
        x.safe_click(item(dock, 2).0, item(dock, 2).1)?;
        wait(200);
        x.safe_click(bx.0 + 75, bx.1 + 22)?;
        wait(1800);
        x.key(0xff1b);
        wait(1200);
        let snapped = marks_since(start)
            .into_iter()
            .rev()
            .find(|(_, m)| m.kind == koon_core::Kind::Area && m.text.is_empty() && m.status.open())
            .map(|(_, m)| m.bounds());
        let ca = x.c.translate_coordinates(card, x.root, 0, 0).ok().and_then(|r| r.reply().ok()).ok_or("card not mapped")?;
        let dock = open_dock(x)?;
        x.safe_click(item(dock, 2).0, item(dock, 2).1)?;
        wait(200);
        x.safe_click(bx.0 + 75, bx.1 + 22)?;
        wait(1500);
        x.motion(bx.0 + 20, bx.1 + 10);
        wait(100);
        let _ = x.c.xtest_fake_input(4, 4, 0, x.root, 0, 0, 0);
        let _ = x.c.xtest_fake_input(5, 4, 0, x.root, 0, 0, 0);
        let _ = x.c.flush();
        wait(400);
        x.key(0xff1b);
        wait(1200);
        let grown = marks_since(start)
            .into_iter()
            .rev()
            .find(|(_, m)| m.kind == koon_core::Kind::Area && m.text.is_empty() && m.status.open())
            .map(|(_, m)| m.bounds());
        let card_at = [(ca.dst_x - ov.0) as f32, (ca.dst_y - ov.1) as f32, 340.0, 180.0];
        check(
            "the mouse wheel grows the selection to the enclosing element",
            grown.is_some_and(|b| b.iter().zip(card_at).all(|(a, e)| (a - e).abs() <= 3.0)),
            format!("got {grown:?} expected {card_at:?}"),
        );
        let _ = x.c.destroy_window(page);
        let _ = x.c.flush();
        for (s, m) in marks_since(start).into_iter().filter(|(_, m)| m.kind == koon_core::Kind::Area && m.text.is_empty()) {
            let _ = Store::open().update(&s.id, |s| {
                if let Some(m) = s.mark_mut(m.id) {
                    m.status = Status::Resolved;
                }
            });
        }
        let _ = koon_core::ipc::send("reload");
        let expect = [(bx.0 - ov.0) as f32, (bx.1 - ov.1) as f32, 150.0, 44.0];
        check(
            "a single click snaps to the element under the pointer",
            snapped.is_some_and(|b| b.iter().zip(expect).all(|(a, e)| (a - e).abs() <= 3.0)),
            format!("got {snapped:?} expected {expect:?}"),
        );

        Ok(())
    }

    pub fn suite_snap() -> Result<(), String> {
        let (c, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = c.setup().roots[screen].root;
        let x = X { c, root };
        let start = koon_core::store::now_ms();
        let wins = x.windows();
        let mut overlays: Vec<_> = wins.iter().filter(|w| w.2.2 > 400).map(|w| (w.0, w.2)).collect();
        overlays.sort_by_key(|o| o.1.0);
        let (_, ov) = *overlays.first().ok_or("no overlay")?;
        let mut check = |name: &str, ok: bool, detail: String| println!("{} {name} — {detail}", if ok { "✅" } else { "❌" });
        snap_check(&x, ov, start, &mut check)?;
        let _ = koon_core::ipc::send("toggle");
        Ok(())
    }

    pub fn suite() -> Result<(), String> {
        let (c, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = c.setup().roots[screen].root;
        let x = X { c, root };
        let start = koon_core::store::now_ms();
        let mut fails = Vec::new();
        let mut check = |name: &str, ok: bool, detail: String| {
            println!("{} {name}{}", if ok { "✅" } else { "❌" }, if detail.is_empty() { String::new() } else { format!(" — {detail}") });
            if !ok {
                fails.push(name.to_string());
            }
        };
        let wins = x.windows();
        let pill = wins.iter().find(|w| w.2.2 == 76).ok_or("pill not found")?.2;
        let mut overlays: Vec<_> = wins.iter().filter(|w| w.2.2 > 400).map(|w| (w.0, w.2)).collect();
        overlays.sort_by_key(|o| o.1.0);
        let ov = overlays[0].1;
        let open_dock = |x: &X| -> Result<(i16, i16), String> {
            let wins = x.windows();
            if let Some(d) = wins.iter().filter(|w| w.2.2 > 400).find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1))) {
                return Ok(d);
            }
            let p = wins.iter().find(|w| w.2.2 == 76).map(|w| w.2).ok_or("no pill")?;
            x.safe_click(p.0 + 38, p.1 + 64)?;
            wait(700);
            x.windows()
                .iter()
                .filter(|w| w.2.2 > 400)
                .find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1)))
                .ok_or_else(|| "the dock did not open".to_string())
        };
        let item = |d: (i16, i16), i: i16| (d.0 + 26, d.1 + 76 + 40 * i + if i >= 6 { 8 } else { 0 });
        let typed_ok = |x: &X, text: &str| -> Result<(), String> {
            if !x.focused_koon() {
                return Err("focus is not on koon; not typing".into());
            }
            x.typing(text);
            Ok(())
        };

        let dock = open_dock(&x)?;
        check(
            "dock opens next to the pill",
            (dock.0 - pill.0 - 12).abs() <= 1 && (dock.1 - pill.1 - 12).abs() <= 1,
            format!("dock {:?} pill {:?}", dock, (pill.0, pill.1)),
        );

        x.safe_click(item(dock, 0).0, item(dock, 0).1)?;
        wait(200);
        let p1 = (ov.0 + 260, ov.1 + 300);
        x.safe_click(p1.0, p1.1)?;
        wait(400);
        typed_ok(&x, "comment one")?;
        x.key(0xff0d);
        wait(900);
        let m = marks_since(start);
        check(
            "commenting saves a pin",
            m.iter().any(|(_, m)| m.kind == koon_core::Kind::Pin && m.text == "comment one" && m.status == Status::Pending),
            format!("{} marks", m.len()),
        );

        x.safe_click(item(dock, 1).0, item(dock, 1).1)?;
        wait(200);
        x.drag((ov.0 + 420, ov.1 + 420), (ov.0 + 600, ov.1 + 470))?;
        wait(400);
        typed_ok(&x, "stroke")?;
        x.key(0xff0d);
        wait(900);
        let m = marks_since(start);
        let stroke = m.iter().find(|(_, m)| m.kind == koon_core::Kind::Stroke);
        check(
            "drawing saves a stroke",
            stroke.is_some_and(|(_, m)| m.text == "stroke" && m.points.len() > 5),
            format!("{:?}", stroke.map(|(_, m)| (m.points.len(), m.text.clone()))),
        );

        x.safe_click(item(dock, 2).0, item(dock, 2).1)?;
        wait(200);
        x.drag((ov.0 + 700, ov.1 + 200), (ov.0 + 900, ov.1 + 330))?;
        wait(400);
        x.key(0xff0d);
        wait(900);
        let m = marks_since(start);
        let area = m.iter().find(|(_, m)| m.kind == koon_core::Kind::Area);
        check(
            "marking an area without text saves the area",
            area.is_some_and(|(_, m)| m.status == Status::Pending && m.bounds()[2] > 150.0),
            format!("{:?}", area.map(|(_, m)| m.bounds())),
        );

        let count = marks_since(start).len();
        x.safe_click(item(dock, 0).0, item(dock, 0).1)?;
        wait(200);
        x.safe_click(ov.0 + 300, ov.1 + 600)?;
        wait(400);
        typed_ok(&x, "nope")?;
        x.escape();
        wait(700);
        let after = marks_since(start);
        check(
            "esc cancels a new comment",
            after.len() == count && !after.iter().any(|(_, m)| m.text.contains("nope")),
            format!("{count} → {}", after.len()),
        );

        let badge = (p1.0 + 13, p1.1 - 13);
        let owns_badge = x.owns(badge.0, badge.1);
        check("the pin badge receives clicks", owns_badge, format!("{badge:?}"));
        if owns_badge {
            x.safe_click(badge.0, badge.1)?;
            wait(400);
            let f = x.c.get_input_focus().ok().and_then(|r| r.reply().ok()).map(|r| r.focus);
            let rects: Vec<_> = x.windows().iter().map(|w| (w.0, w.3.clone())).collect();
            println!("   debug focus {f:?} windows {rects:?}");
            typed_ok(&x, " edited")?;
            x.key(0xff0d);
            wait(1000);
            let m = marks_since(start);
            check(
                "replying adds a message to the pin thread",
                m.iter()
                    .any(|(_, m)| m.text == "comment one" && m.thread.last().is_some_and(|e| e.text == "edited" && e.author == koon_core::Author::User) && m.status == Status::Pending),
                m.iter()
                    .map(|(_, m)| format!("{} {:?}", m.text, m.thread.iter().map(|e| e.text.clone()).collect::<Vec<_>>()))
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
        }

        x.escape();
        wait(500);
        let wins = x.windows();
        let dock_open = wins.iter().any(|w| w.3.iter().any(|r| r.2 == 52));
        check("esc closes the dock", !dock_open, String::new());
        let pill_visible = wins.iter().find(|w| w.2.2 == 76).is_some_and(|w| !w.3.is_empty());
        check("the pill accepts clicks again", pill_visible, String::new());
        let passive: Vec<_> = wins.iter().filter(|w| w.2.2 > 400).flat_map(|w| w.3.clone()).collect();
        check(
            "passive: only badges receive input",
            !passive.is_empty() && passive.iter().all(|r| r.2 <= 64 && r.3 <= 64),
            format!("{passive:?}"),
        );

        let open_before = marks_since(start).iter().filter(|(_, m)| m.status.open()).count();
        for s in latest(6).iter().filter(|s| s.created >= start && s.open().next().is_some()) {
            let _ = Store::open().update(&s.id, |s| {
                for m in s.marks.iter_mut() {
                    m.status = Status::Resolved;
                }
            });
        }
        let _ = koon_core::ipc::send("reload");
        wait(1500);
        let wins = x.windows();
        let passive: Vec<_> = wins.iter().filter(|w| w.2.2 > 400).flat_map(|w| w.3.clone()).collect();
        check(
            "resolving clears the marks from the screen",
            passive.is_empty(),
            format!("{open_before} open before; input {passive:?}"),
        );

        let _ = koon_core::ipc::send("toggle");
        wait(600);
        let opened = x.windows().iter().any(|w| w.3.iter().any(|r| r.2 == 52));
        let _ = koon_core::ipc::send("toggle");
        wait(600);
        let closed = !x.windows().iter().any(|w| w.3.iter().any(|r| r.2 == 52));
        check("koon toggle opens and closes", opened && closed, format!("opened {opened} closed {closed}"));

        let dock = open_dock(&x)?;
        x.safe_click(dock.0 + 26, dock.1 + 30)?;
        wait(600);
        check("clicking the logo collapses the dock", !x.windows().iter().any(|w| w.3.iter().any(|r| r.2 == 52)), String::new());

        let dock = open_dock(&x)?;
        x.safe_click(item(dock, 0).0, item(dock, 0).1)?;
        wait(200);
        x.safe_click(ov.0 + 300, ov.1 + 520)?;
        wait(400);
        typed_ok(&x, "half a thou")?;
        wait(300);
        let typing = koon_core::ipc::is_typing();
        x.key(0xff0d);
        wait(700);
        for (s, m) in marks_since(start).into_iter().filter(|(_, m)| m.text == "half a thou") {
            let _ = Store::open().update(&s.id, |s| {
                if let Some(m) = s.mark_mut(m.id) {
                    m.status = Status::Resolved;
                }
            });
        }
        let _ = koon_core::ipc::send("reload");
        check(
            "the agent sees the user typing",
            typing && !koon_core::ipc::is_typing(),
            format!("typing {typing} after {}", koon_core::ipc::is_typing()),
        );
        let _ = koon_core::ipc::send("toggle");
        wait(600);

        let dock = open_dock(&x)?;
        x.safe_click(item(dock, 0).0, item(dock, 0).1)?;
        wait(200);
        x.safe_click(ov.0 + 220, ov.1 + 560)?;
        wait(400);
        typed_ok(&x, "first in a row")?;
        x.shift_key(0xff0d);
        wait(500);
        x.safe_click(ov.0 + 360, ov.1 + 600)?;
        wait(400);
        typed_ok(&x, "second in a row")?;
        x.key(0xff0d);
        wait(900);
        let row: Vec<_> = marks_since(start).into_iter().filter(|(_, m)| m.text.ends_with("in a row")).collect();
        check("shift+enter saves and keeps commenting", row.len() == 2, format!("{} saved", row.len()));

        if std::env::var_os("KOON_VOICE_FILE").is_some() {
            let dock = open_dock(&x)?;
            x.safe_click(item(dock, 5).0, item(dock, 5).1)?;
            wait(200);
            x.safe_click(ov.0 + 520, ov.1 + 520)?;
            wait(1200);
            x.key(0xff0d);
            let mut heard = None;
            for _ in 0..60 {
                wait(250);
                x.key(0xff0d);
                wait(300);
                heard = marks_since(start).into_iter().map(|(_, m)| m.text).find(|t| t.contains("país") || t.contains("country"));
                if heard.is_some() {
                    break;
                }
            }
            check("dictating fills the comment with what was said", heard.is_some(), format!("{heard:?}"));
        }
        for (s, m) in row {
            let _ = Store::open().update(&s.id, |s| {
                if let Some(m) = s.mark_mut(m.id) {
                    m.status = Status::Resolved;
                }
            });
        }
        let _ = koon_core::ipc::send("reload");
        let _ = koon_core::ipc::send("toggle");
        wait(600);

        let dock = open_dock(&x)?;
        x.safe_click(item(dock, 0).0, item(dock, 0).1)?;
        wait(200);
        let pin = (ov.0 + 520, ov.1 + 640);
        x.safe_click(pin.0, pin.1)?;
        wait(400);
        typed_ok(&x, "reopen me")?;
        x.key(0xff0d);
        wait(900);
        let _ = koon_core::ipc::send("toggle");
        wait(600);
        if let Some((s, m)) = marks_since(start).into_iter().find(|(_, m)| m.text == "reopen me") {
            let _ = Store::open().update(&s.id, |s| {
                if let Some(m) = s.mark_mut(m.id) {
                    m.status = Status::Resolved;
                    m.note = Some("listo".into());
                    m.thread.push(koon_core::Entry::new(koon_core::Author::Agent, koon_core::Say::Done, "listo"));
                }
            });
            let _ = koon_core::ipc::send("reload");
            wait(900);
            x.safe_click(pin.0 + 26 + 6 + 20, pin.1 - 13)?;
            wait(500);
            typed_ok(&x, "no asi no")?;
            x.key(0xff0d);
            wait(900);
            let after = Store::open().load(&s.id).ok().and_then(|s| s.mark(m.id).cloned());
            let ok = after
                .as_ref()
                .is_some_and(|m| m.status == Status::Pending && m.last().is_some_and(|e| e.author == koon_core::Author::User && e.text == "no asi no"));
            check("clicking the agent's ✓ reopens the mark with a reply", ok, format!("{:?}", after.map(|m| (m.status, m.thread.len()))));
            let _ = Store::open().update(&s.id, |s| {
                if let Some(m) = s.mark_mut(m.id) {
                    m.status = Status::Resolved;
                }
            });
            let _ = koon_core::ipc::send("reload");
            wait(3500);
        } else {
            check("clicking the agent's ✓ reopens the mark with a reply", false, "no mark".into());
        }

        let p0 = x.windows().iter().find(|w| w.2.2 == 76).map(|w| w.2).ok_or("no pill")?;
        x.drag((p0.0 + 38, p0.1 + 30), (p0.0 - 82, p0.1 - 50))?;
        wait(700);
        let p1 = x.windows().iter().find(|w| w.2.2 == 76).map(|w| w.2).ok_or("no pill")?;
        let saved = Store::open().prefs().pill;
        check(
            "the pill remembers where it was dropped",
            saved.is_some_and(|(sx, sy)| (sx - p1.0 as i32).abs() <= 2 && (sy - p1.1 as i32).abs() <= 2) && (p1.0 - p0.0 + 120).abs() <= 4,
            format!("moved {:?} -> {:?}, saved {saved:?}", (p0.0, p0.1), (p1.0, p1.1)),
        );
        x.drag((p1.0 + 38, p1.1 + 30), (p0.0 + 38, p0.1 + 30))?;
        wait(700);

        snap_check(&x, ov, start, &mut check)?;

        let store = Store::open();
        let dock = open_dock(&x)?;
        x.safe_click(item(dock, 3).0, item(dock, 3).1)?;
        wait(200);
        x.drag((ov.0 + 140, ov.1 + 140), (ov.0 + 340, ov.1 + 260))?;
        wait(400);
        typed_ok(&x, "suite memory: from the suite")?;
        x.key(0xff0d);
        wait(1800);
        let saved = store.memory("suite-memory");
        let leftover = marks_since(start).iter().any(|(_, m)| m.text.contains("suite memory"));
        check(
            "saving a memory stores the cropped image, not a mark",
            saved.as_ref().is_some_and(|m| m.note == "from the suite" && m.size[0] >= 190 && m.size[1] >= 110) && !leftover,
            format!("{saved:?} leftover mark {leftover}"),
        );

        let _ = koon_core::ipc::send("memories");
        wait(700);
        typed_ok(&x, "suite mem")?;
        x.key(0xff0d);
        wait(300);
        typed_ok(&x, "use it here")?;
        x.key(0xff0d);
        wait(1200);
        let used = marks_since(start)
            .iter()
            .any(|(_, m)| m.memory.as_deref() == saved.as_ref().map(|m| m.id.as_str()) && m.text == "use it here" && m.status == Status::Pending);
        check("picking a memory from the palette sends it to the agent", used, String::new());
        if let Some(m) = &saved {
            let _ = store.delete_memory(&m.id);
        }
        for (s, _) in marks_since(start) {
            let _ = store.update(&s.id, |s| s.marks.iter_mut().for_each(|m| m.status = Status::Resolved));
        }
        let _ = koon_core::ipc::send("reload");
        let _ = koon_core::ipc::send("toggle");
        wait(600);

        let empty = Store::open().list().into_iter().filter(|s| s.created >= start && s.marks.is_empty()).count();
        check("no empty sessions left", empty == 0, format!("{empty} empty"));
        println!("\n{} failures: {:?}", fails.len(), fails);
        Ok(())
    }

    fn app_windows(x: &X, needle: &str) -> Vec<(i16, i16, u16, u16)> {
        let name = atom(&x.c, "_NET_WM_NAME");
        let utf8 = atom(&x.c, "UTF8_STRING");
        let list = atom(&x.c, "_NET_CLIENT_LIST");
        let ids: Vec<u32> =
            x.c.get_property(false, x.root, list, AtomEnum::WINDOW, 0, 1024)
                .ok()
                .and_then(|r| r.reply().ok())
                .and_then(|r| r.value32().map(|v| v.collect()))
                .unwrap_or_default();
        let mut out = Vec::new();
        for w in ids {
            let title =
                x.c.get_property(false, w, name, utf8, 0, 256)
                    .ok()
                    .and_then(|r| r.reply().ok())
                    .map(|r| String::from_utf8_lossy(&r.value).to_string())
                    .unwrap_or_default();
            if !title.contains(needle) {
                continue;
            }
            let Some(g) = x.c.get_geometry(w).ok().and_then(|r| r.reply().ok()) else { continue };
            let Some(t) = x.c.translate_coordinates(w, x.root, 0, 0).ok().and_then(|r| r.reply().ok()) else {
                continue;
            };
            out.push((t.dst_x, t.dst_y, g.width, g.height));
        }
        out.sort_by_key(|a| a.0);
        out
    }

    #[allow(dead_code)]
    fn app_window(x: &X, needle: &str) -> Option<(i16, i16, u16, u16)> {
        let name = atom(&x.c, "_NET_WM_NAME");
        let utf8 = atom(&x.c, "UTF8_STRING");
        let mut stack = x.c.query_tree(x.root).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
        let mut best: Option<(i16, i16, u16, u16)> = None;
        while let Some(w) = stack.pop() {
            if let Some(t) = x.c.query_tree(w).ok().and_then(|r| r.reply().ok()) {
                stack.extend(t.children);
            }
            let title =
                x.c.get_property(false, w, name, utf8, 0, 256)
                    .ok()
                    .and_then(|r| r.reply().ok())
                    .map(|r| String::from_utf8_lossy(&r.value).to_string())
                    .unwrap_or_default();
            if !title.contains(needle) {
                continue;
            }
            let Some(g) = x.c.get_geometry(w).ok().and_then(|r| r.reply().ok()) else { continue };
            let Some(t) = x.c.translate_coordinates(w, x.root, 0, 0).ok().and_then(|r| r.reply().ok()) else {
                continue;
            };
            if g.width > 300 && best.is_none_or(|b| g.width as u32 * g.height as u32 > b.2 as u32 * b.3 as u32) {
                best = Some((t.dst_x, t.dst_y, g.width, g.height));
            }
        }
        best
    }

    fn bubble_trash(pin: (i16, i16), screen: (i16, i16, u16, u16)) -> (i16, i16) {
        let (lx, ly) = ((pin.0 - screen.0) as f32, (pin.1 - screen.1) as f32);
        let anchor = (lx, ly - 26.0, 26.0f32, 26.0f32);
        let (w, h) = (320.0, 84.0);
        let mut bx = anchor.0 + anchor.2 + 10.0;
        if bx + w > screen.2 as f32 - 12.0 {
            bx = anchor.0 - 10.0 - w;
        }
        let by = (anchor.1 - 6.0).clamp(12.0, screen.3 as f32 - h - 12.0);
        (screen.0 + (bx.max(12.0) + w - 40.0 + 13.0) as i16, screen.1 + (by + 52.0 + 13.0) as i16)
    }

    pub fn stress(rounds: usize) -> Result<(), String> {
        let (c, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = c.setup().roots[screen].root;
        let x = X { c, root };
        let apps = app_windows(&x, "Café Orbital");
        if apps.is_empty() {
            return Err("no Café Orbital X11 windows found".into());
        }
        println!("Café Orbital at {:?}", apps);
        let wins = x.windows();
        let desk = wins.iter().filter(|w| w.2.2 > 400).map(|w| w.2).next().ok_or("no overlay")?;
        let mut xs: Vec<(i32, i32)> = crate::mutter::connectors().iter().map(|(p, _)| *p).collect();
        xs.sort();
        let overlays: Vec<(i16, i16, u16, u16)> = xs
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let right = xs.get(k + 1).map(|n| n.0).unwrap_or(desk.0 as i32 + desk.2 as i32);
                (p.0 as i16, p.1 as i16, (right - p.0) as u16, desk.3)
            })
            .collect();
        println!("monitors {:?}", overlays);
        let screen_of = |p: (i16, i16)| overlays.iter().copied().find(|o| p.0 >= o.0 && p.0 < o.0 + o.2 as i16 && p.1 >= o.1 && p.1 < o.1 + o.3 as i16);
        let at = |fx: f32, fy: f32| (fx, fy);
        let targets = [
            ("the moon", at(0.30, 0.42), "the moon spins on click"),
            ("the title", at(0.16, 0.12), "brighter logo"),
            ("the menu", at(0.70, 0.30), "more spacing in the menu"),
            ("zorp", at(0.20, 0.82), "zorp with an animated avatar"),
            ("the table", at(0.50, 0.85), "highlight my row"),
            ("achievements", at(0.82, 0.86), "bigger achievements"),
            ("the wallet", at(0.85, 0.12), "gold wallet"),
            ("the score", at(0.30, 0.66), "animated score"),
        ];
        let start = koon_core::store::now_ms();
        let mut fails: Vec<String> = Vec::new();
        let mut note = |name: String, ok: bool, detail: String| {
            println!("{} {name}{}", if ok { "✅" } else { "❌" }, if detail.is_empty() { String::new() } else { format!(" — {detail}") });
            if !ok {
                fails.push(name);
            }
        };
        let dock_rect = |x: &X| {
            x.windows()
                .iter()
                .filter(|w| w.2.2 > 400)
                .find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1)))
        };
        let pill_rect = |x: &X| x.windows().iter().find(|w| w.2.2 == 76).map(|w| w.2);
        let open = |x: &X| -> Result<(i16, i16), String> {
            if let Some(d) = dock_rect(x) {
                return Ok(d);
            }
            let p = pill_rect(x).ok_or("no pill")?;
            x.safe_click(p.0 + 38, p.1 + 64)?;
            for _ in 0..20 {
                wait(60);
                if let Some(d) = dock_rect(x) {
                    return Ok(d);
                }
            }
            Err("the dock did not open".into())
        };
        let comment = |x: &X, d: (i16, i16), p: (i16, i16), text: &str| -> Result<u128, String> {
            x.safe_click(d.0 + 26, d.1 + 76)?;
            wait(150);
            x.safe_click(p.0, p.1)?;
            wait(250);
            if !x.focused_koon() {
                let f = x.c.get_input_focus().ok().and_then(|r| r.reply().ok()).map(|r| r.focus);
                x.escape();
                return Err(format!("focus left koon after marking ({f:?})"));
            }
            x.typing(text);
            let t = std::time::Instant::now();
            x.key(0xff0d);
            for _ in 0..60 {
                wait(50);
                if Store::open()
                    .list()
                    .iter()
                    .any(|s| s.created >= start && s.marks.iter().any(|m| m.text == text && m.status != Status::Draft) && s.annotated.is_some())
                {
                    return Ok(t.elapsed().as_millis());
                }
            }
            Err("not saved within 3 s".into())
        };

        for round in 0..rounds {
            println!("\n— round {} —", round + 1);
            let d = open(&x)?;
            let mut times = Vec::new();
            for (k, (name, f, text)) in targets.iter().enumerate() {
                let app = apps[(k + round) % apps.len()];
                let p = &(app.0 + (app.2 as f32 * f.0) as i16 + 36 * round as i16, app.1 + (app.3 as f32 * f.1) as i16 + 18 * round as i16);
                if screen_of(*p).is_none() {
                    continue;
                }
                let text = format!("{text} {round}");
                match comment(&x, d, *p, &text) {
                    Ok(ms) => times.push(ms),
                    Err(e) => note(format!("comment on {name} (round {})", round + 1), false, e),
                }
            }
            let slow = times.iter().copied().max().unwrap_or(0);
            note(
                format!("burst of {} comments (round {})", times.len(), round + 1),
                times.len() == targets.len(),
                format!("saved in {:?} ms", times),
            );
            let expected: std::collections::BTreeSet<String> = crate::mutter::connectors()
                .into_iter()
                .filter(|(p, _)| apps.iter().any(|a| a.0 as i32 >= p.0 && (a.0 as i32) < p.0 + 1366))
                .map(|(_, n)| n)
                .collect();
            let got: std::collections::BTreeSet<String> = Store::open()
                .list()
                .iter()
                .filter(|s| s.created >= start)
                .flat_map(|s| {
                    s.marks
                        .iter()
                        .filter(|m| m.text.ends_with(&format!(" {round}")))
                        .filter_map(|m| s.monitor_at(m.points[0]).map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .collect();
            note(
                format!("each comment stays on the monitor where it was made (round {})", round + 1),
                got == expected,
                format!("{got:?}"),
            );
            note(format!("each comment saves in < 1.5 s (round {})", round + 1), slow < 1500, format!("worst {slow} ms"));

            let (name, f, _) = targets[0];
            let app = apps[round % apps.len()];
            let p = (app.0 + (app.2 as f32 * f.0) as i16 + 36 * round as i16, app.1 + (app.3 as f32 * f.1) as i16 + 18 * round as i16);
            let trash = bubble_trash(p, screen_of(p).unwrap_or(overlays[0]));
            let before = Store::open()
                .list()
                .iter()
                .filter(|s| s.created >= start)
                .flat_map(|s| s.marks.iter().filter(|m| m.status.open()))
                .count();
            if x.owns(p.0 + 13, p.1 - 13) {
                x.safe_click(p.0 + 13, p.1 - 13)?;
                wait(300);
                let ok_trash = x.owns(trash.0, trash.1);
                if ok_trash {
                    x.safe_click(trash.0, trash.1)?;
                    wait(900);
                }
                let after = Store::open()
                    .list()
                    .iter()
                    .filter(|s| s.created >= start)
                    .flat_map(|s| s.marks.iter().filter(|m| m.status.open()))
                    .count();
                note(
                    format!("delete {name} with the trash button (round {})", round + 1),
                    ok_trash && after + 1 == before,
                    format!("{before} → {after} open; trash at {trash:?} clickable {ok_trash}"),
                );
            } else {
                note(format!("badge of {name} clickable (round {})", round + 1), false, String::new());
            }

            x.escape();
            wait(400);
            for i in 0..6 {
                let o = open(&x);
                let d = dock_rect(&x);
                if let (Ok(_), Some(d)) = (o, d) {
                    x.safe_click(d.0 + 26, d.1 + 30)?;
                    wait(250);
                    if dock_rect(&x).is_some() {
                        note(format!("open/close #{i} (round {})", round + 1), false, "the logo did not collapse".into());
                        break;
                    }
                } else {
                    note(format!("open/close #{i} (round {})", round + 1), false, "did not open".into());
                    break;
                }
            }

            let p0 = pill_rect(&x).ok_or("no pill")?;
            let other = overlays.iter().copied().find(|o| !(p0.0 >= o.0 && p0.0 < o.0 + o.2 as i16)).unwrap_or(overlays[0]);
            let dest = (other.0 + other.2 as i16 - 140, other.1 + 200);
            x.drag((p0.0 + 38, p0.1 + 64), (dest.0 + 38, dest.1 + 64))?;
            wait(300);
            let p1 = pill_rect(&x).ok_or("no pill")?;
            let moved = (p1.0 - dest.0).abs() <= 1 && (p1.1 - dest.1).abs() <= 1;
            note(
                format!("move the pill to the other monitor (round {})", round + 1),
                moved,
                format!("{:?} → {:?}, expected {:?}", (p0.0, p0.1), (p1.0, p1.1), dest),
            );
            let d = open(&x)?;
            let near = (d.0 - p1.0 - 12).abs() <= 2 && (d.1 - p1.1 - 12).abs() <= 40;
            note(
                format!("the dock appears where the pill is (round {})", round + 1),
                near,
                format!("dock {:?} pill {:?}", d, (p1.0, p1.1)),
            );
            x.escape();
            wait(500);
            x.drag((p1.0 + 38, p1.1 + 64), (p0.0 + 38, p0.1 + 64))?;
            wait(300);
        }

        let open_now: Vec<_> = Store::open().list().into_iter().filter(|s| s.created >= start && s.open().next().is_some()).collect();
        let badges: usize = x.windows().iter().filter(|w| w.2.2 > 400).map(|w| w.3.iter().filter(|r| r.2 <= 64 && r.3 <= 64).count()).sum();
        let marks: usize = open_now.iter().map(|s| s.open().count()).sum();
        note("every open mark has a clickable badge".into(), badges >= marks, format!("{marks} marks, {badges} zones"));
        for s in &open_now {
            let _ = Store::open().update(&s.id, |s| s.marks.iter_mut().for_each(|m| m.status = Status::Resolved));
        }
        let _ = koon_core::ipc::send("reload");
        wait(1500);
        let left: usize = x.windows().iter().filter(|w| w.2.2 > 400).map(|w| w.3.len()).sum();
        note("resolving everything clears the screen".into(), left == 0, format!("{left} input zones left"));
        println!("\n{} failures", fails.len());
        for f in &fails {
            println!("  · {f}");
        }
        Ok(())
    }

    pub fn mark(specs: &[String]) -> Result<(), String> {
        let (c, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = c.setup().roots[screen].root;
        let x = X { c, root };
        let apps = app_windows(&x, "Café Orbital");
        let app = *apps.last().ok_or("no Café Orbital X11 window found")?;
        let p = x.windows().iter().find(|w| w.2.2 == 76).map(|w| w.2).ok_or("no pill")?;
        let dock = x
            .windows()
            .iter()
            .filter(|w| w.2.2 > 400)
            .find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1)));
        let d = match dock {
            Some(d) => d,
            None => {
                x.safe_click(p.0 + 38, p.1 + 64)?;
                wait(700);
                x.windows()
                    .iter()
                    .filter(|w| w.2.2 > 400)
                    .find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1)))
                    .ok_or("no dock")?
            }
        };
        for spec in specs {
            let (action, spec) = match spec.split_once(':') {
                Some((a @ ("edit" | "delete" | "open"), rest)) => (a, rest),
                _ => ("new", spec.as_str()),
            };
            let mut parts = spec.splitn(3, ',');
            let fx: f32 = parts.next().and_then(|v| v.parse().ok()).ok_or("format fx,fy,text")?;
            let fy: f32 = parts.next().and_then(|v| v.parse().ok()).ok_or("format fx,fy,text")?;
            let text = parts.next().unwrap_or("").trim();
            let pt = (app.0 + (app.2 as f32 * fx) as i16, app.1 + (app.3 as f32 * fy) as i16);
            if action != "new" {
                x.safe_click(pt.0 + 13, pt.1 - 13)?;
                wait(350);
                if action == "open" {
                    println!("opened at {pt:?}");
                    continue;
                }
                if action == "delete" {
                    let screen = crate::mutter::connectors()
                        .iter()
                        .map(|(p, _)| *p)
                        .filter(|p| p.0 <= pt.0 as i32)
                        .max()
                        .map(|p| (p.0 as i16, p.1 as i16, 1366u16, 768u16))
                        .unwrap_or((0, 0, 1366, 768));
                    let t = bubble_trash(pt, screen);
                    x.safe_click(t.0, t.1)?;
                    println!("deleted at {pt:?}");
                } else {
                    x.typing(text);
                    x.key(0xff0d);
                    println!("edited {text:?} at {pt:?}");
                }
                wait(600);
                continue;
            }
            x.safe_click(d.0 + 26, d.1 + 76)?;
            wait(150);
            x.safe_click(pt.0, pt.1)?;
            wait(300);
            if !x.focused_koon() {
                return Err("focus left koon".into());
            }
            x.typing(text);
            x.key(0xff0d);
            wait(500);
            println!("marked {text:?} at {pt:?}");
        }
        if !specs.iter().any(|s| s.starts_with("open:")) {
            x.escape();
        }
        Ok(())
    }

    pub fn scene() -> Result<(), String> {
        let (c, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = c.setup().roots[screen].root;
        let x = X { c, root };
        let apps = app_windows(&x, "Café Orbital");
        let app = *apps.last().ok_or("no window")?;
        let at = |fx: f32, fy: f32| (app.0 + (app.2 as f32 * fx) as i16, app.1 + (app.3 as f32 * fy) as i16);
        let p = x.windows().iter().find(|w| w.2.2 == 76).map(|w| w.2).ok_or("no pill")?;
        x.safe_click(p.0 + 38, p.1 + 64)?;
        wait(700);
        let d = x
            .windows()
            .iter()
            .filter(|w| w.2.2 > 400)
            .find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1)))
            .ok_or("no dock")?;
        for (pt, text) in [(at(0.30, 0.40), "the moon spins on click"), (at(0.62, 0.30), "more spacing between drinks in the menu")] {
            x.safe_click(d.0 + 26, d.1 + 76)?;
            wait(150);
            x.safe_click(pt.0, pt.1)?;
            wait(300);
            x.typing(text);
            x.key(0xff0d);
            wait(500);
        }
        x.safe_click(d.0 + 26, d.1 + 116)?;
        wait(150);
        x.drag(at(0.40, 0.80), at(0.55, 0.92))?;
        wait(300);
        x.typing("move the table");
        x.key(0xff0d);
        wait(500);
        x.safe_click(d.0 + 26, d.1 + 76)?;
        wait(150);
        let pt = at(0.80, 0.85);
        x.safe_click(pt.0, pt.1)?;
        wait(300);
        x.typing("achievements with a gold glow");
        x.motion(d.0 + 26, d.1 + 116);
        wait(700);
        Ok(())
    }

    pub fn run() -> Result<(), String> {
        let (c, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = c.setup().roots[screen].root;
        let x = X { c, root };
        let before = Store::open().list().len();
        let wins = x.windows();
        let pill = wins.iter().find(|w| w.2.2 == 76).ok_or("pill not found")?.2;
        let mut overlays: Vec<_> = wins.iter().filter(|w| w.2.2 > 400).map(|w| (w.0, w.2)).collect();
        overlays.sort_by_key(|o| o.1.0);
        println!(
            "pill at ({},{}); overlays: {:?}",
            pill.0,
            pill.1,
            overlays.iter().map(|o| (o.1.0, o.1.1, o.1.2, o.1.3)).collect::<Vec<_>>()
        );
        x.safe_click(pill.0 + 38, pill.1 + 64)?;
        wait(700);
        let wins = x.windows();
        let dock = wins
            .iter()
            .filter(|w| w.2.2 > 400)
            .find_map(|w| w.3.iter().find(|r| r.2 == 52).map(|r| (w.2.0 + r.0, w.2.1 + r.1)))
            .ok_or("the dock did not open")?;
        println!("dock at ({},{})", dock.0, dock.1);
        let comment = (dock.0 + 26, dock.1 + 76);
        for (k, o) in overlays.iter().enumerate() {
            x.safe_click(comment.0, comment.1)?;
            wait(250);
            let target = (o.1.0 + 300 + 40 * k as i16, o.1.1 + 260);
            x.safe_click(target.0, target.1)?;
            wait(450);
            if !x.focused_koon() {
                let _ = koon_core::ipc::send("toggle");
                return Err(format!("keyboard focus is not on koon after marking on monitor {k}; not typing anything"));
            }
            x.typing(&format!("test {k}"));
            x.key(0xff0d);
            wait(900);
        }
        let _ = koon_core::ipc::send("toggle");
        wait(600);
        let store = Store::open();
        let all = store.list();
        println!("new sessions: {}", all.len().saturating_sub(before));
        for s in all.iter().rev().take(overlays.len() + 1) {
            for m in s.marks.iter().filter(|m| m.status != Status::Draft) {
                let img = store.path_of(&s.id, s.annotated.as_deref()).map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0));
                println!(
                    "  monitor {:?} mark #{} {:?} at ({:.0},{:.0}) status {:?} capture {:?} bytes",
                    s.monitor, m.id, m.text, m.points[0][0], m.points[0][1], m.status, img
                );
            }
        }
        Ok(())
    }
}

pub fn suite_snap() {
    if let Err(e) = e2e::suite_snap() {
        println!("suite aborted: {e}");
    }
}

pub fn suite() {
    match e2e::suite() {
        Ok(()) => println!("suite finished"),
        Err(e) => println!("suite aborted: {e}"),
    }
}

pub fn stress(rounds: usize) {
    match e2e::stress(rounds) {
        Ok(()) => println!("stress finished"),
        Err(e) => println!("stress aborted: {e}"),
    }
}

pub fn scene() {
    match e2e::scene() {
        Ok(()) => {}
        Err(e) => println!("scene aborted: {e}"),
    }
}

pub fn mark(specs: &[String]) {
    match e2e::mark(specs) {
        Ok(()) => {}
        Err(e) => println!("mark aborted: {e}"),
    }
}

pub fn e2e() {
    if let Err(e) = e2e::run() {
        println!("e2e failed: {e}");
    }
}

#[cfg(target_os = "linux")]
pub fn detect_probe(out: &str) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, CreateWindowAux, WindowClass};
    let run = || -> Result<(), String> {
        let (c, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = c.setup().roots[screen].root;
        let mk = |parent, r: (i16, i16, u16, u16), color: u32| -> Result<u32, String> {
            let id = c.generate_id().map_err(|e| e.to_string())?;
            let aux = CreateWindowAux::new().background_pixel(color).override_redirect(1);
            c.create_window(x11rb::COPY_DEPTH_FROM_PARENT, id, parent, r.0, r.1, r.2, r.3, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
                .map_err(|e| e.to_string())?;
            c.map_window(id).map_err(|e| e.to_string())?;
            Ok(id)
        };
        let page = mk(root, (420, 300, 420, 260), 0xf5f5f7)?;
        let card = mk(page, (40, 40, 340, 180), 0xffffff)?;
        let _ = mk(card, (60, 110, 150, 44), 0x3b6fe0)?;
        c.flush().map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(900));
        let known = crate::mutter::connectors();
        println!("connectors {known:?}");
        let t = std::time::Instant::now();
        let shots = crate::mutter::grab(&known.iter().map(|k| k.1.clone()).collect::<Vec<_>>())?;
        println!("grab {:?}", t.elapsed());
        let _ = c.destroy_window(page);
        let _ = c.flush();
        for ((at, name), shot) in known.iter().zip(shots) {
            let Some(img) = shot else { continue };
            println!("{name} at {at:?} {}x{}", img.w, img.h);
            let (x, y) = ((420 + 40 + 60 + 75 - at.0).max(0) as u32, (300 + 40 + 110 + 22 - at.1).max(0) as u32);
            if x < img.w && y < img.h {
                let t = std::time::Instant::now();
                println!("boxes {:?} in {:?}", koon_core::detect::boxes(&img, (x, y)), t.elapsed());
                std::fs::write(out, img.encode_png().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    };
    if let Err(e) = run() {
        println!("detect probe failed: {e}");
    }
}
