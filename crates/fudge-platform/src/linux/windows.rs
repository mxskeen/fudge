//! Window listing / focusing: X11 (EWMH via x11rb), Hyprland, Sway.

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::util::run_capture;
use crate::{DesktopKind, SessionKind, WindowId, WindowInfo};

const NEEDS_EXTENSION: &str = "window switching on this desktop needs the Fudge extension";

pub fn list(desktop: DesktopKind, session: SessionKind) -> Result<Vec<WindowInfo>> {
    match (session, desktop) {
        (SessionKind::X11, _) => x11::list(),
        (_, DesktopKind::Hyprland) => hypr_list(),
        (_, DesktopKind::Sway) => sway_list(),
        // TODO(kde): KWin script over D-Bus. TODO(gnome): bundled Shell extension.
        // XWayland windows could be listed via X11 but would miss native Wayland ones.
        _ => bail!(NEEDS_EXTENSION),
    }
}

pub fn focus(desktop: DesktopKind, session: SessionKind, id: &WindowId) -> Result<()> {
    match (session, desktop) {
        (SessionKind::X11, _) => x11::focus(id),
        (_, DesktopKind::Hyprland) => {
            run_capture("hyprctl", &["dispatch", "focuswindow", &format!("address:{}", id.0)]).map(|_| ())
        }
        (_, DesktopKind::Sway) => {
            run_capture("swaymsg", &[&format!("[con_id={}] focus", id.0)]).map(|_| ())
        }
        _ => bail!(NEEDS_EXTENSION),
    }
}

pub(crate) fn parse_hypr_clients(json: &str) -> Result<Vec<WindowInfo>> {
    let v: Value = serde_json::from_str(json).context("bad hyprctl output")?;
    let arr = v.as_array().context("hyprctl clients: expected array")?;
    let mut out = Vec::new();
    for c in arr {
        if c.get("mapped").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let Some(address) = c.get("address").and_then(Value::as_str) else { continue };
        let title = c.get("title").and_then(Value::as_str).unwrap_or("").to_string();
        let class = c.get("class").and_then(Value::as_str).unwrap_or("").to_string();
        if title.is_empty() && class.is_empty() {
            continue;
        }
        out.push(WindowInfo {
            id: WindowId(address.to_string()),
            title,
            app_name: class.clone(),
            app_id: (!class.is_empty()).then(|| class.to_lowercase()),
            pid: c.get("pid").and_then(Value::as_u64).map(|p| p as u32),
        });
    }
    Ok(out)
}

fn hypr_list() -> Result<Vec<WindowInfo>> {
    parse_hypr_clients(&run_capture("hyprctl", &["clients", "-j"])?)
}

pub(crate) fn parse_sway_tree(json: &str) -> Result<Vec<WindowInfo>> {
    fn walk(n: &Value, out: &mut Vec<WindowInfo>) {
        let is_leaf_window = n.get("pid").is_some()
            && n.get("nodes").and_then(Value::as_array).map_or(true, |a| a.is_empty())
            && matches!(n.get("type").and_then(Value::as_str), Some("con") | Some("floating_con"));
        if is_leaf_window {
            let class = n
                .get("app_id")
                .and_then(Value::as_str)
                .or_else(|| n.pointer("/window_properties/class").and_then(Value::as_str))
                .unwrap_or("")
                .to_string();
            if let Some(id) = n.get("id").and_then(Value::as_i64) {
                out.push(WindowInfo {
                    id: WindowId(id.to_string()),
                    title: n.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                    app_name: class.clone(),
                    app_id: (!class.is_empty()).then(|| class.to_lowercase()),
                    pid: n.get("pid").and_then(Value::as_u64).map(|p| p as u32),
                });
            }
        }
        for key in ["nodes", "floating_nodes"] {
            if let Some(a) = n.get(key).and_then(Value::as_array) {
                for c in a {
                    walk(c, out);
                }
            }
        }
    }
    let v: Value = serde_json::from_str(json).context("bad swaymsg output")?;
    let mut out = Vec::new();
    walk(&v, &mut out);
    Ok(out)
}

fn sway_list() -> Result<Vec<WindowInfo>> {
    parse_sway_tree(&run_capture("swaymsg", &["-t", "get_tree"])?)
}

mod x11 {
    use anyhow::{Context, Result};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        Atom, AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window,
    };
    use x11rb::rust_connection::RustConnection;

    use crate::{WindowId, WindowInfo};

    struct Atoms {
        client_list: Atom,
        net_wm_name: Atom,
        utf8: Atom,
        net_wm_pid: Atom,
        wm_class: Atom,
        window_type: Atom,
        type_normal: Atom,
        type_dialog: Atom,
    }

    fn atom(conn: &RustConnection, name: &str) -> Result<Atom> {
        Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
    }

    fn atoms(conn: &RustConnection) -> Result<Atoms> {
        Ok(Atoms {
            client_list: atom(conn, "_NET_CLIENT_LIST")?,
            net_wm_name: atom(conn, "_NET_WM_NAME")?,
            utf8: atom(conn, "UTF8_STRING")?,
            net_wm_pid: atom(conn, "_NET_WM_PID")?,
            wm_class: AtomEnum::WM_CLASS.into(),
            window_type: atom(conn, "_NET_WM_WINDOW_TYPE")?,
            type_normal: atom(conn, "_NET_WM_WINDOW_TYPE_NORMAL")?,
            type_dialog: atom(conn, "_NET_WM_WINDOW_TYPE_DIALOG")?,
        })
    }

    pub fn list() -> Result<Vec<WindowInfo>> {
        let (conn, screen_num) = x11rb::connect(None).context("cannot connect to the X server")?;
        let root = conn.setup().roots[screen_num].root;
        let a = atoms(&conn)?;
        let reply = conn
            .get_property(false, root, a.client_list, AtomEnum::WINDOW, 0, 4096)?
            .reply()?;
        let wins: Vec<Window> = reply.value32().map(|i| i.collect()).unwrap_or_default();
        let mut out = Vec::new();
        for w in wins {
            // Skip docks, panels, desktops, tooltips: keep normal/dialog (or untyped) windows.
            if let Some(types) = conn
                .get_property(false, w, a.window_type, AtomEnum::ATOM, 0, 32)?
                .reply()
                .ok()
                .and_then(|r| r.value32().map(|i| i.collect::<Vec<u32>>()))
            {
                if !types.is_empty() && !types.iter().any(|t| *t == a.type_normal || *t == a.type_dialog) {
                    continue;
                }
            }
            let title = conn
                .get_property(false, w, a.net_wm_name, a.utf8, 0, 1024)?
                .reply()
                .ok()
                .map(|r| String::from_utf8_lossy(&r.value).into_owned())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    conn.get_property(false, w, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)
                        .ok()?
                        .reply()
                        .ok()
                        .map(|r| String::from_utf8_lossy(&r.value).into_owned())
                })
                .unwrap_or_default();
            // WM_CLASS is "instance\0class\0".
            let class = conn
                .get_property(false, w, a.wm_class, AtomEnum::STRING, 0, 256)?
                .reply()
                .ok()
                .map(|r| {
                    let parts: Vec<String> = r
                        .value
                        .split(|b| *b == 0)
                        .filter(|p| !p.is_empty())
                        .map(|p| String::from_utf8_lossy(p).into_owned())
                        .collect();
                    (parts.first().cloned(), parts.get(1).cloned())
                })
                .unwrap_or((None, None));
            let pid = conn
                .get_property(false, w, a.net_wm_pid, AtomEnum::CARDINAL, 0, 1)?
                .reply()
                .ok()
                .and_then(|r| r.value32().and_then(|mut i| i.next()));
            if title.is_empty() && class.1.is_none() {
                continue;
            }
            out.push(WindowInfo {
                id: WindowId(w.to_string()),
                title,
                app_name: class.1.clone().or(class.0.clone()).unwrap_or_default(),
                app_id: class.0.map(|s| s.to_lowercase()),
                pid,
            });
        }
        Ok(out)
    }

    pub fn focus(id: &WindowId) -> Result<()> {
        let window: Window = id.0.parse().context("bad X11 window id")?;
        let (conn, screen_num) = x11rb::connect(None).context("cannot connect to the X server")?;
        let root = conn.setup().roots[screen_num].root;
        let active = atom(&conn, "_NET_ACTIVE_WINDOW")?;
        // source indication 2 = pager, which most WMs let steal focus.
        let ev = ClientMessageEvent::new(32, window, active, [2u32, 0, 0, 0, 0]);
        conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            ev,
        )?;
        conn.flush()?;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyprctl_fixture() {
        let j = r#"[{"address":"0x5581a0","mapped":true,"title":"Docs - Firefox","class":"firefox","pid":4242},
                    {"address":"0x5581b0","mapped":false,"title":"x","class":"y","pid":1}]"#;
        let w = parse_hypr_clients(j).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].id.0, "0x5581a0");
        assert_eq!(w[0].app_name, "firefox");
        assert_eq!(w[0].pid, Some(4242));
    }

    #[test]
    fn sway_fixture() {
        let j = r#"{"id":1,"type":"root","nodes":[{"id":2,"type":"output","nodes":[
            {"id":3,"type":"workspace","nodes":[
              {"id":10,"type":"con","name":"term","app_id":"foot","pid":77,"nodes":[]},
              {"id":11,"type":"con","name":"Slack","pid":78,"window_properties":{"class":"Slack"},"nodes":[]}
            ],"floating_nodes":[{"id":12,"type":"floating_con","name":"calc","app_id":"gnome-calculator","pid":79,"nodes":[]}]}
        ]}]}"#;
        let w = parse_sway_tree(j).unwrap();
        assert_eq!(w.len(), 3);
        assert_eq!(w[1].app_name, "Slack");
        assert_eq!(w[2].id.0, "12");
    }
}
