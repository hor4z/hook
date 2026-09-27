use koon_core::{Author, Entry, Kind, Memory, Say, Session, Status, Store, ipc};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_CROPS: usize = 8;
const MAX_MEMORIES: usize = 4;

const QUIET: Duration = Duration::from_millis(1500);

fn tools() -> Value {
    json!([
        { "name": "koon_list_feedback", "description": "List open feedback the user left on screen with koon (pins, freehand strokes, areas), grouped by capture session, with text and coordinates. Cheap: no images.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "koon_get_feedback", "description": "Get one feedback session: the annotated screenshot, a zoomed crop per mark and the marks as JSON (logical and physical pixel coordinates). Marks become `taken`. Defaults to the oldest session with pending marks.", "inputSchema": { "type": "object", "properties": { "session": { "type": "string" } } } },
        { "name": "koon_wait_feedback", "description": "Block until the user finishes a batch of feedback in koon (closes the dock, or stops commenting for a moment and is not typing), then return it like koon_get_feedback. Use it to wait for the user's next instruction.", "inputSchema": { "type": "object", "properties": { "timeout_s": { "type": "integer", "minimum": 1, "maximum": 3600 } } } },
        { "name": "koon_ask", "description": "Ask the user a question about one mark when the request is ambiguous. It is added to the mark's conversation thread and shown on screen; the user's answer comes back as a pending mark through koon_wait_feedback, with the whole thread.", "inputSchema": { "type": "object", "properties": { "session": { "type": "string" }, "id": { "type": "integer" }, "question": { "type": "string" } }, "required": ["session", "id", "question"] } },
        { "name": "koon_reply", "description": "Add a message to a mark's conversation thread without closing it (e.g. what you are about to do, or a partial answer).", "inputSchema": { "type": "object", "properties": { "session": { "type": "string" }, "id": { "type": "integer" }, "text": { "type": "string" } }, "required": ["session", "id", "text"] } },
        { "name": "koon_list_memories", "description": "List the user's koon memories: visual references (a button, a card, a layout...) they saved from any screen to say \"make it like this\". Cheap: names and notes, no images.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "koon_move_memory", "description": "Put a memory in a folder (path like \"botones/web\"; empty string takes it out of any folder). Folders organize the user's memories in koon's palette; use it when asked to sort or group them.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "path": { "type": "string" } }, "required": ["id", "path"] } },
        { "name": "koon_get_memory", "description": "Get one memory by id or name: its image at full resolution plus the user's note. Use it as an exact visual reference and adapt it to the project's code and design tokens.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "koon_resolve", "description": "Mark feedback as resolved after applying it; the mark fades out of the user's screen. Omit `ids` to resolve every open mark of the session.", "inputSchema": { "type": "object", "properties": { "session": { "type": "string" }, "ids": { "type": "array", "items": { "type": "integer" } }, "note": { "type": "string" } }, "required": ["session"] } }
    ])
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn text(t: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": t.into() }] })
}

fn failure(t: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": t.into() }], "isError": true })
}

fn kind(k: Kind) -> &'static str {
    match k {
        Kind::Pin => "pin",
        Kind::Stroke => "stroke",
        Kind::Area => "area",
    }
}

fn status(s: Status) -> &'static str {
    match s {
        Status::Draft => "draft",
        Status::Pending => "pending",
        Status::Taken => "taken",
        Status::Resolved => "resolved",
    }
}

fn describe(s: &Session) -> String {
    let mut out = format!(
        "session {} · monitor {} · screen {}x{} logical @{}x\n",
        s.id,
        s.monitor.as_deref().unwrap_or("?"),
        s.size[0],
        s.size[1],
        s.scale
    );
    for m in s.open() {
        let b = m.bounds();
        let at = if m.kind == Kind::Pin {
            format!("at ({:.0},{:.0})", b[0], b[1])
        } else {
            format!("box ({:.0},{:.0} {:.0}x{:.0})", b[0], b[1], b[2], b[3])
        };
        let at = format!("{at} on {}", s.monitor_at(m.points[0]).unwrap_or("?"));
        let say = if m.text.trim().is_empty() { "(no text)".to_string() } else { format!("\"{}\"", m.text.trim()) };
        out.push_str(&format!("  #{} {} {} {} {}\n", m.id, kind(m.kind), at, status(m.status), say));
        for e in &m.thread {
            out.push_str(&format!("      {}: {}\n", if e.author == Author::User { "user" } else { "agent" }, e.text.trim()));
        }
    }
    out
}

fn marks_json(s: &Session) -> Value {
    let marks: Vec<Value> = s
        .open()
        .map(|m| {
            let b = m.bounds();
            json!({
                "id": m.id,
                "kind": kind(m.kind),
                "monitor": s.monitor_at(m.points[0]),
                "text": m.text,
                "status": m.status,
                "bounds": b,
                "bounds_px": [b[0] * s.scale, b[1] * s.scale, b[2] * s.scale, b[3] * s.scale],
                "points": if m.points.len() > 64 { m.points.iter().step_by(m.points.len() / 64 + 1).copied().collect::<Vec<_>>() } else { m.points.clone() },
                "audio": m.audio.as_ref().map(|a| s.id.clone() + "/" + a),
                "thread": m.thread.iter().map(|e| json!({ "from": if e.author == Author::User { "user" } else { "agent" }, "say": e.say, "text": e.text })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "session": s.id, "monitor": s.monitor, "screen": { "logical": s.size, "scale": s.scale }, "marks": marks })
}

fn memory_line(m: &Memory) -> String {
    let note = if m.note.is_empty() { String::new() } else { format!(" — \"{}\"", m.note) };
    let colours = if m.palette.is_empty() { String::new() } else { format!(" · palette {}", m.palette.join(", ")) };
    let folder = if m.path.is_empty() { String::new() } else { format!(" in {}/", m.path) };
    format!("@{} «{}»{folder} {}x{}{note}{colours}", m.id, m.name, m.size[0], m.size[1])
}

fn notify() {
    let _ = ipc::send("reload");
}

struct Server {
    store: Store,
}

impl Server {
    fn open(&self) -> Vec<Session> {
        self.store.list().into_iter().filter(|s| s.open().next().is_some()).collect()
    }

    fn pick(&self, id: Option<&str>) -> Option<Session> {
        match id {
            Some(id) => self.store.load(id).ok(),
            None => {
                let open = self.open();
                open.iter().find(|s| s.marks.iter().any(|m| m.status == Status::Pending)).or(open.last()).cloned()
            }
        }
    }

    fn deliver(&self, s: Session) -> Value {
        let mut content = vec![json!({ "type": "text", "text": format!("{}\n{}\nApply the requested changes, then call koon_resolve with this session id.", describe(&s), marks_json(&s)) })];
        let png = |name: &str| std::fs::read(self.store.file(&s.id, name)).ok();
        if let Some(bytes) = s.preview.as_deref().or(s.annotated.as_deref()).and_then(png) {
            content.push(json!({ "type": "image", "data": base64(&bytes), "mimeType": "image/png" }));
        }
        for m in s.open().take(MAX_CROPS) {
            if let Some(bytes) = png(&format!("crop-{}.png", m.id)) {
                content.push(json!({ "type": "text", "text": format!("crop of #{}", m.id) }));
                content.push(json!({ "type": "image", "data": base64(&bytes), "mimeType": "image/png" }));
            }
        }
        let mut refs: Vec<(u32, Memory)> = Vec::new();
        for m in s.open() {
            let named = m.memory.as_deref().and_then(|id| self.store.memory(id));
            let texts = std::iter::once(m.text.as_str()).chain(m.thread.iter().filter(|e| e.author == Author::User).map(|e| e.text.as_str()));
            for mem in named.into_iter().chain(texts.flat_map(|t| self.store.mentioned(t))) {
                if !refs.iter().any(|(_, r)| r.id == mem.id) {
                    refs.push((m.id, mem));
                }
            }
        }
        for (mid, mem) in refs.into_iter().take(MAX_MEMORIES) {
            if let Ok(bytes) = std::fs::read(self.store.memory_file(&mem.id, "image.png")) {
                content.push(json!({ "type": "text", "text": format!("memory referenced by #{mid}: {} — reproduce this reference, adapted to the project", memory_line(&mem)) }));
                content.push(json!({ "type": "image", "data": base64(&bytes), "mimeType": "image/png" }));
            }
        }
        let _ = self.store.update(&s.id, |s| {
            for m in s.marks.iter_mut().filter(|m| m.status == Status::Pending) {
                m.status = Status::Taken;
            }
        });
        notify();
        json!({ "content": content })
    }

    fn call(&mut self, name: &str, args: &Value) -> Value {
        let s = |k: &str| args.get(k).and_then(Value::as_str);
        match name {
            "koon_list_feedback" => {
                let open = self.open();
                if open.is_empty() {
                    return text("no open feedback. Ask the user to mark the screen with koon (Ctrl+Alt+K) or call koon_wait_feedback.");
                }
                text(open.iter().map(describe).collect::<Vec<_>>().join("\n"))
            }
            "koon_get_feedback" => match self.pick(s("session")) {
                Some(session) if session.open().next().is_some() => self.deliver(session),
                Some(session) => text(format!("session {} has no open marks", session.id)),
                None => text("no open feedback"),
            },
            "koon_wait_feedback" => {
                let timeout = Duration::from_secs(args.get("timeout_s").and_then(Value::as_u64).unwrap_or(600).clamp(1, 3600));
                let start = Instant::now();
                let mut seen: Option<(usize, Instant)> = None;
                loop {
                    let pending: Vec<Session> = self.store.list().into_iter().filter(|s| s.marks.iter().any(|m| m.status == Status::Pending)).collect();
                    let count: usize = pending.iter().map(|s| s.marks.iter().filter(|m| m.status == Status::Pending).count()).sum();
                    if count > 0 {
                        let since = match seen {
                            Some((n, t)) if n == count => t,
                            _ => Instant::now(),
                        };
                        seen = Some((count, since));
                        let settled = !ipc::is_typing() && since.elapsed() >= QUIET;
                        if !ipc::is_open() || settled || start.elapsed() >= timeout {
                            let session = pending.into_iter().next().expect("pending");
                            return self.deliver(session);
                        }
                    }
                    if start.elapsed() >= timeout {
                        return text("no new feedback before the timeout");
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            "koon_ask" | "koon_reply" => {
                let (Some(sid), Some(mid), Some(msg)) = (s("session"), args.get("id").and_then(Value::as_u64), s("question").or(s("text"))) else {
                    return failure("session, id and question/text are required");
                };
                let say = if name == "koon_ask" { Say::Question } else { Say::Reply };
                let msg = msg.to_string();
                match self.store.update(sid, |s| {
                    if let Some(m) = s.mark_mut(mid as u32) {
                        m.status = Status::Taken;
                        m.thread.push(Entry::new(Author::Agent, say, msg.clone()));
                    }
                }) {
                    Ok(_) => {
                        notify();
                        text(if say == Say::Question {
                            format!("asked on #{mid}; the user's reply comes back through koon_wait_feedback")
                        } else {
                            format!("replied on #{mid}")
                        })
                    }
                    Err(e) => failure(format!("cannot update session {sid}: {e}")),
                }
            }
            "koon_resolve" => {
                let Some(id) = s("session") else { return failure("session is required") };
                let ids: Option<Vec<u32>> = args.get("ids").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|v| v as u32).collect());
                let note = s("note").map(str::to_string);
                match self.store.update(id, |s| {
                    for m in s.marks.iter_mut().filter(|m| m.status.open() && ids.as_ref().is_none_or(|ids| ids.contains(&m.id))) {
                        m.status = Status::Resolved;
                        m.note = note.clone();
                        if let Some(n) = &note {
                            m.thread.push(Entry::new(Author::Agent, Say::Done, n.clone()));
                        }
                    }
                }) {
                    Ok(s) => {
                        notify();
                        let left = s.open().count();
                        text(format!("resolved; {left} open mark(s) left in session {}", s.id))
                    }
                    Err(e) => failure(format!("cannot update session {id}: {e}")),
                }
            }
            "koon_list_memories" => {
                let all = self.store.memories();
                if all.is_empty() {
                    return text("no memories yet. The user saves them from koon's dock (Guardar memory) by marking any part of the screen.");
                }
                text(all.iter().map(memory_line).collect::<Vec<_>>().join("\n"))
            }
            "koon_move_memory" => {
                let Some(key) = s("id") else { return failure("id is required") };
                let Some(mem) = self.store.memory(key) else { return failure(format!("no memory named {key}")) };
                match self.store.move_memory(&mem.id, s("path").unwrap_or("")) {
                    Ok(m) => {
                        notify();
                        text(memory_line(&m))
                    }
                    Err(e) => failure(format!("cannot move {}: {e}", mem.id)),
                }
            }
            "koon_get_memory" => {
                let Some(key) = s("id") else { return failure("id is required") };
                let Some(mem) = self.store.memory(key) else { return failure(format!("no memory named {key}")) };
                let mut content = vec![json!({ "type": "text", "text": memory_line(&mem) })];
                if let Ok(bytes) = std::fs::read(self.store.memory_file(&mem.id, "image.png")) {
                    content.push(json!({ "type": "image", "data": base64(&bytes), "mimeType": "image/png" }));
                }
                json!({ "content": content })
            }
            other => failure(format!("unknown tool {other}")),
        }
    }

    fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let result = match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(VERSIONS[0]);
                let version = if VERSIONS.contains(&asked) { asked } else { VERSIONS[0] };
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "koon", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "koon lets the user point at their screen: pins with comments, freehand strokes and areas over a screenshot. They can also save memories (visual references from any screen) and mention them as @name; referenced memories come attached to the feedback. Call koon_get_feedback (or koon_wait_feedback to wait for the next one), look at the annotated screenshot and crops, map each mark to the code that renders it, apply the change, then koon_resolve."
                })
            }
            "ping" => json!({}),
            "tools/list" => json!({ "tools": tools() }),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                self.call(name, &args)
            }
            _ if id.is_none() => return None,
            other => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("method not found: {other}") } })),
        };
        id.map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

fn main() {
    let mut server = Server { store: Store::open() };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => server.handle(&msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(r) = reply
            && (writeln!(out, "{r}").is_err() || out.flush().is_err())
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koon_core::Mark;

    fn server(tag: &str) -> Server {
        let root = std::env::temp_dir().join(format!("koon-mcp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Server { store: Store::at(root) }
    }

    fn seed(store: &Store) -> Session {
        let mut s = store.create([1000.0, 800.0], 2.0).unwrap();
        let mut m = Mark::new(1, Kind::Pin, [100.0, 50.0]);
        m.text = "Bigger Button".into();
        m.status = Status::Pending;
        s.marks.push(m);
        store.save(&s).unwrap();
        s
    }

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(&[0xff, 0xee, 0xdd, 0xcc]), "/+7dzA==");
    }

    #[test]
    fn initialize_and_list_tools() {
        let mut srv = server("init");
        let r = srv
            .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }))
            .unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        let r = srv.handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
        assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 9);
        assert!(srv.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
    }

    #[test]
    fn get_takes_marks_and_resolve_closes_them() {
        let mut srv = server("flow");
        let s = seed(&srv.store);
        let listed = srv.call("koon_list_feedback", &json!({}));
        let t = listed["content"][0]["text"].as_str().unwrap();
        assert!(t.contains("#1 pin at (100,50) on ? pending \"Bigger Button\""), "{t}");
        let got = srv.call("koon_get_feedback", &json!({}));
        assert!(got["content"][0]["text"].as_str().unwrap().contains("\"bounds_px\":[200.0,100.0,0.0,0.0]"));
        assert_eq!(srv.store.load(&s.id).unwrap().marks[0].status, Status::Taken);
        srv.call("koon_resolve", &json!({ "session": s.id, "note": "listo" }));
        let after = srv.store.load(&s.id).unwrap();
        assert_eq!(after.marks[0].status, Status::Resolved);
        assert_eq!(after.marks[0].note.as_deref(), Some("listo"));
        assert!(srv.call("koon_list_feedback", &json!({}))["content"][0]["text"].as_str().unwrap().starts_with("no open feedback"));
        let _ = std::fs::remove_dir_all(&srv.store.root);
    }

    #[test]
    fn wait_returns_immediately_when_feedback_is_pending() {
        let mut srv = server("wait");
        seed(&srv.store);
        let r = srv.call("koon_wait_feedback", &json!({ "timeout_s": 1 }));
        assert!(r["content"][0]["text"].as_str().unwrap().contains("Bigger Button"));
        let r = srv.call("koon_wait_feedback", &json!({ "timeout_s": 1 }));
        assert_eq!(r["content"][0]["text"], "no new feedback before the timeout");
        let _ = std::fs::remove_dir_all(&srv.store.root);
    }

    #[test]
    fn memories_are_listed_fetched_and_attached_when_mentioned() {
        let mut srv = server("memories");
        assert!(srv.call("koon_list_memories", &json!({}))["content"][0]["text"].as_str().unwrap().starts_with("no memories yet"));
        let img = koon_core::Image::new(30, 10);
        srv.store.save_memory("Tarjeta Linear", "el borde suave", &img, None).unwrap();
        let list = srv.call("koon_list_memories", &json!({}));
        assert_eq!(list["content"][0]["text"], "@tarjeta-linear «Tarjeta Linear» 30x10 — \"el borde suave\"");
        let moved = srv.call("koon_move_memory", &json!({ "id": "tarjeta-linear", "path": "tarjetas/" }));
        assert_eq!(moved["content"][0]["text"], "@tarjeta-linear «Tarjeta Linear» in tarjetas/ 30x10 — \"el borde suave\"");
        srv.call("koon_move_memory", &json!({ "id": "tarjeta-linear", "path": "" }));
        let got = srv.call("koon_get_memory", &json!({ "id": "Tarjeta Linear" }));
        assert_eq!(got["content"][1]["type"], "image");
        assert_eq!(srv.call("koon_get_memory", &json!({ "id": "nada" }))["isError"], true);

        let mut s = srv.store.create([1000.0, 800.0], 1.0).unwrap();
        let mut m = Mark::new(1, Kind::Pin, [10.0, 10.0]);
        m.text = "que este botón sea como @tarjeta-linear".into();
        m.status = Status::Pending;
        s.marks.push(m);
        let mut picked = Mark::new(2, Kind::Pin, [20.0, 20.0]);
        picked.text = "quiero esto en el header".into();
        picked.memory = Some("tarjeta-linear".into());
        picked.status = Status::Pending;
        s.marks.push(picked);
        srv.store.save(&s).unwrap();
        let fb = srv.call("koon_get_feedback", &json!({}));
        let items = fb["content"].as_array().unwrap();
        let refs: Vec<&str> = items.iter().filter_map(|c| c["text"].as_str()).filter(|t| t.starts_with("memory referenced")).collect();
        assert_eq!(refs.len(), 1, "{refs:?}");
        assert!(refs[0].contains("@tarjeta-linear"));
        assert_eq!(items.last().unwrap()["type"], "image");
        let _ = std::fs::remove_dir_all(&srv.store.root);
    }
}
