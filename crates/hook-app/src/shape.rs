use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, SK, SO};
use x11rb::protocol::xproto::{ClipOrdering, Rectangle};
use x11rb::rust_connection::RustConnection;

pub struct Shaper {
    conn: RustConnection,
}

fn xid(window: &Window) -> Option<u32> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(h) => Some(h.window as u32),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}

impl Shaper {
    pub fn new() -> Option<Shaper> {
        let (conn, _) = x11rb::connect(None).ok()?;
        Some(Shaper { conn })
    }

    pub fn pointer(&self) -> Option<(i32, i32)> {
        self.pointer_state().map(|(p, _)| p)
    }

    pub fn pointer_state(&self) -> Option<((i32, i32), bool)> {
        use x11rb::protocol::xproto::{ConnectionExt, KeyButMask};
        let root = self.conn.setup().roots.first()?.root;
        let r = self.conn.query_pointer(root).ok()?.reply().ok()?;
        Some(((r.root_x as i32, r.root_y as i32), r.mask.contains(KeyButMask::BUTTON1)))
    }

    pub fn input(&self, window: &Window, rects: &[(f32, f32, f32, f32)]) -> bool {
        let Some(id) = xid(window) else { return false };
        let s = window.scale_factor() as f32;
        let rects: Vec<Rectangle> = rects
            .iter()
            .map(|r| Rectangle {
                x: (r.0 * s).floor() as i16,
                y: (r.1 * s).floor() as i16,
                width: (r.2 * s).ceil().max(0.0) as u16,
                height: (r.3 * s).ceil().max(0.0) as u16,
            })
            .collect();
        let ok = shape::rectangles(&self.conn, SO::SET, SK::INPUT, ClipOrdering::UNSORTED, id, 0, 0, &rects).is_ok();
        ok && self.conn.flush().is_ok()
    }
}
