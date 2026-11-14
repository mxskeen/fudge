//! X11 window placement: GTK4 cannot position toplevels, so we ask the X server directly.

use gdk4_x11::prelude::*;
use gtk4::gdk;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt};
use x11rb::rust_connection::RustConnection;

pub struct Placer {
    conn: RustConnection,
    root: u32,
}

impl Placer {
    /// None when the display is not X11.
    pub fn new() -> Option<Placer> {
        let display = gdk::Display::default()?;
        if !display.is::<gdk4_x11::X11Display>() {
            return None;
        }
        let (conn, n) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[n].root;
        Some(Placer { conn, root })
    }

    /// Centre horizontally on the monitor under the pointer, top edge at 1/5 of its height.
    pub fn place(&self, window: &impl IsA<gtk4::Window>, width: i32) {
        let Some(surface) = gtk4::prelude::NativeExt::surface(window.as_ref()) else { return };
        let Ok(xs) = surface.downcast::<gdk4_x11::X11Surface>() else { return };
        let Some(display) = gdk::Display::default() else { return };
        let Some(ptr) = self.conn.query_pointer(self.root).ok().and_then(|c| c.reply().ok()) else { return };

        let monitors = display.monitors();
        let n = monitors.n_items();
        let mon = |i: u32| monitors.item(i).and_then(|o| o.downcast::<gdk::Monitor>().ok());
        let scale = mon(0).map(|m| m.scale_factor()).unwrap_or(1).max(1);
        let (px, py) = (ptr.root_x as i32 / scale, ptr.root_y as i32 / scale);
        let mut target = mon(0);
        for i in 0..n {
            if let Some(m) = mon(i) {
                let g = m.geometry();
                if px >= g.x() && px < g.x() + g.width() && py >= g.y() && py < g.y() + g.height() {
                    target = Some(m);
                    break;
                }
            }
        }
        let Some(m) = target else { return };
        let g = m.geometry();
        let x = (g.x() + (g.width() - width) / 2) * scale;
        let y = (g.y() + g.height() / 5) * scale;
        let _ = self.conn.configure_window(xs.xid() as u32, &ConfigureWindowAux::new().x(x).y(y));
        let _ = self.conn.flush();
    }
}

