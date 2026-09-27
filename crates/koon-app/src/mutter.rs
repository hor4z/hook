use koon_core::Image;
use pipewire as pw;
use pw::spa;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, Value};

const DEST: &str = "org.gnome.Mutter.ScreenCast";

fn err<E: std::fmt::Display>(what: &'static str) -> impl Fn(E) -> String {
    move |e| format!("mutter {what}: {e}")
}

pub fn grab(connectors: &[String]) -> Result<Vec<Option<Image>>, String> {
    let conn = Connection::session().map_err(err("bus"))?;
    let root = Proxy::new(&conn, DEST, "/org/gnome/Mutter/ScreenCast", "org.gnome.Mutter.ScreenCast").map_err(err("proxy"))?;
    let props: HashMap<&str, Value> = HashMap::from([("disable-animations", Value::Bool(true))]);
    let path: OwnedObjectPath = root.call("CreateSession", &(props,)).map_err(err("session"))?;
    let session = Proxy::new(&conn, DEST, path.as_str(), "org.gnome.Mutter.ScreenCast.Session").map_err(err("session"))?;
    let mut waits = Vec::new();
    for c in connectors {
        let opts: HashMap<&str, Value> = HashMap::from([("cursor-mode", Value::U32(0))]);
        let stream: Result<OwnedObjectPath, _> = session.call("RecordMonitor", &(c.as_str(), opts));
        match stream {
            Ok(sp) => {
                let proxy = Proxy::new(&conn, DEST, sp.as_str().to_owned(), "org.gnome.Mutter.ScreenCast.Stream").map_err(err("stream"))?;
                let signals = proxy.receive_signal("PipeWireStreamAdded").map_err(err("signal"))?;
                waits.push(Some((proxy, signals)));
            }
            Err(e) => {
                eprintln!("koon: could not record {c}: {e}");
                waits.push(None);
            }
        }
    }
    let _: () = session.call("Start", &()).map_err(err("start"))?;
    let nodes: Vec<Option<u32>> = waits
        .iter_mut()
        .map(|w| w.as_mut().and_then(|(_, s)| s.next()).and_then(|m| m.body().deserialize::<(u32,)>().ok()).map(|(n,)| n))
        .collect();
    let result = frames(&nodes);
    let _: Result<(), _> = session.call("Stop", &());
    result
}

struct Data {
    index: usize,
    format: spa::param::video::VideoInfoRaw,
    out: Rc<RefCell<Vec<Option<Image>>>>,
    left: Rc<RefCell<usize>>,
    quit: pw::main_loop::MainLoopRc,
}

fn convert(src: &[u8], w: u32, h: u32, stride: usize, format: spa::param::video::VideoFormat) -> Option<Image> {
    use spa::param::video::VideoFormat as F;
    let order: [usize; 3] = match format {
        F::BGRx | F::BGRA => [2, 1, 0],
        F::RGBx | F::RGBA => [0, 1, 2],
        _ => return None,
    };
    if h == 0 || stride < w as usize * 4 || src.len() < stride * (h as usize - 1) + w as usize * 4 {
        return None;
    }
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        for p in src[y * stride..y * stride + w as usize * 4].chunks_exact(4) {
            rgba.extend_from_slice(&[p[order[0]], p[order[1]], p[order[2]], 255]);
        }
    }
    Some(Image { w, h, rgba })
}

fn format_pod() -> Result<Vec<u8>, String> {
    use spa::param::video::VideoFormat;
    use spa::pod::{Value as PodValue, property};
    let obj = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        property!(spa::param::format::FormatProperties::MediaType, Id, spa::param::format::MediaType::Video),
        property!(spa::param::format::FormatProperties::MediaSubtype, Id, spa::param::format::MediaSubtype::Raw),
        property!(
            spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::BGRx,
            VideoFormat::BGRx,
            VideoFormat::BGRA,
            VideoFormat::RGBx,
            VideoFormat::RGBA
        ),
        property!(
            spa::param::format::FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            spa::utils::Rectangle { width: 1920, height: 1080 },
            spa::utils::Rectangle { width: 1, height: 1 },
            spa::utils::Rectangle { width: 16384, height: 16384 }
        ),
        property!(
            spa::param::format::FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            spa::utils::Fraction { num: 30, denom: 1 },
            spa::utils::Fraction { num: 0, denom: 1 },
            spa::utils::Fraction { num: 60, denom: 1 }
        ),
    );
    Ok(spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &PodValue::Object(obj))
        .map_err(err("pod"))?
        .0
        .into_inner())
}

fn frames(nodes: &[Option<u32>]) -> Result<Vec<Option<Image>>, String> {
    pw::init();
    let e = err("pipewire");
    let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(&e)?;
    let context = pw::context::ContextRc::new(&mainloop, None).map_err(&e)?;
    let core = context.connect_rc(None).map_err(&e)?;
    let pod = format_pod()?;
    let out = Rc::new(RefCell::new(vec![None; nodes.len()]));
    let left = Rc::new(RefCell::new(nodes.iter().flatten().count()));
    let mut keep = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(node) = node else { continue };
        let data = Data {
            index,
            format: Default::default(),
            out: out.clone(),
            left: left.clone(),
            quit: mainloop.clone(),
        };
        let stream = pw::stream::StreamBox::new(
            &core,
            "koon",
            pw::properties::properties! {
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Screen",
            },
        )
        .map_err(&e)?;
        let listener = stream
            .add_local_listener_with_user_data(data)
            .param_changed(|_, d, id, param| {
                let Some(param) = param else { return };
                if id == spa::param::ParamType::Format.as_raw() {
                    let _ = d.format.parse(param);
                }
            })
            .process(|stream, d| {
                let Some(mut buffer) = stream.dequeue_buffer() else { return };
                if d.out.borrow()[d.index].is_some() {
                    return;
                }
                let datas = buffer.datas_mut();
                let Some(first) = datas.first_mut() else { return };
                let (size, offset, stride) = (first.chunk().size() as usize, first.chunk().offset() as usize, first.chunk().stride());
                let (w, h) = (d.format.size().width, d.format.size().height);
                if size == 0 || w == 0 {
                    return;
                }
                let Some(bytes) = first.data() else { return };
                let end = (offset + size).min(bytes.len());
                let stride = if stride > 0 { stride as usize } else { w as usize * 4 };
                if let Some(img) = convert(&bytes[offset.min(end)..end], w, h, stride, d.format.format()) {
                    d.out.borrow_mut()[d.index] = Some(img);
                    *d.left.borrow_mut() -= 1;
                    if *d.left.borrow() == 0 {
                        d.quit.quit();
                    }
                }
            })
            .register()
            .map_err(&e)?;
        let mut params = [spa::pod::Pod::from_bytes(&pod).ok_or("mutter: bad pod")?];
        stream
            .connect(
                spa::utils::Direction::Input,
                Some(*node),
                pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
                &mut params,
            )
            .map_err(&e)?;
        keep.push((stream, listener));
    }
    if *left.borrow() > 0 {
        let quit = mainloop.clone();
        let timer = mainloop.loop_().add_timer(move |_| quit.quit());
        let _ = timer.update_timer(Some(Duration::from_millis(2500)), None);
        mainloop.run();
    }
    for (s, _) in &keep {
        let _ = s.disconnect();
    }
    let v = out.borrow().clone();
    Ok(v)
}

type MonitorSpec = (String, String, String, String);
type Mode = (String, i32, i32, f64, f64, Vec<f64>, HashMap<String, zbus::zvariant::OwnedValue>);
type Physical = (MonitorSpec, Vec<Mode>, HashMap<String, zbus::zvariant::OwnedValue>);
type Logical = (i32, i32, f64, u32, bool, Vec<MonitorSpec>, HashMap<String, zbus::zvariant::OwnedValue>);

pub fn connectors() -> Vec<((i32, i32), String)> {
    let Ok(conn) = Connection::session() else { return Vec::new() };
    let Ok(p) = Proxy::new(&conn, "org.gnome.Mutter.DisplayConfig", "/org/gnome/Mutter/DisplayConfig", "org.gnome.Mutter.DisplayConfig") else {
        return Vec::new();
    };
    let state: Result<(u32, Vec<Physical>, Vec<Logical>, HashMap<String, zbus::zvariant::OwnedValue>), _> = p.call("GetCurrentState", &());
    state
        .map(|(_, _, logical, _)| logical.into_iter().filter_map(|l| l.5.first().map(|m| ((l.0, l.1), m.0.clone()))).collect())
        .unwrap_or_default()
}

pub fn pick(known: &[((i32, i32), String)], at: (i32, i32), index: usize, count: usize) -> Option<String> {
    if let Some((_, c)) = known.iter().find(|(p, _)| *p == at) {
        return Some(c.clone());
    }
    if known.len() == count {
        let mut sorted = known.to_vec();
        sorted.sort_by_key(|(p, _)| (p.0, p.1));
        return sorted.get(index).map(|(_, c)| c.clone());
    }
    None
}
