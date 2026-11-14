//! Hotkey string parsing (portable) and the platform grab implementations.

use std::fmt;

pub const MOD_SHIFT: u8 = 1;
pub const MOD_CTRL: u8 = 2;
pub const MOD_ALT: u8 = 4;
pub const MOD_SUPER: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkey {
    pub mods: u8,
    /// Lower-case key name: "space", "k", "f5", "return", ...
    pub key: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct HotkeyError(pub String);

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for HotkeyError {}

impl Hotkey {
    /// Parse "Alt+space", "Ctrl+Alt+k", "Super+space" (case-insensitive, `-` also allowed).
    pub fn parse(s: &str) -> Result<Hotkey, HotkeyError> {
        let mut mods = 0u8;
        let mut key: Option<String> = None;
        for part in s.split(['+', '-']).map(str::trim).filter(|p| !p.is_empty()) {
            let lower = part.to_lowercase();
            let m = match lower.as_str() {
                "alt" | "option" | "opt" | "mod1" => MOD_ALT,
                "ctrl" | "control" | "ctl" => MOD_CTRL,
                "shift" => MOD_SHIFT,
                "super" | "win" | "meta" | "cmd" | "command" | "mod4" => MOD_SUPER,
                _ => 0,
            };
            if m != 0 {
                mods |= m;
            } else if key.replace(lower).is_some() {
                return Err(HotkeyError(format!("hotkey {s:?} has more than one non-modifier key")));
            }
        }
        // "Alt+-" style: a lone separator key is not supported; keep it simple.
        let key = key.ok_or_else(|| HotkeyError(format!("hotkey {s:?} has no key")))?;
        if mods == 0 {
            return Err(HotkeyError(format!("hotkey {s:?} needs at least one modifier")));
        }
        if keysym(&key).is_none() {
            return Err(HotkeyError(format!("unknown key {key:?} in hotkey {s:?}")));
        }
        Ok(Hotkey { mods, key })
    }

    /// GNOME accelerator syntax, e.g. `<Alt>space`.
    pub fn to_gnome_accel(&self) -> String {
        let mut out = String::new();
        if self.mods & MOD_CTRL != 0 {
            out.push_str("<Control>");
        }
        if self.mods & MOD_ALT != 0 {
            out.push_str("<Alt>");
        }
        if self.mods & MOD_SHIFT != 0 {
            out.push_str("<Shift>");
        }
        if self.mods & MOD_SUPER != 0 {
            out.push_str("<Super>");
        }
        out.push_str(&self.key);
        out
    }

    /// X11 modifier mask (Shift=1, Control=4, Mod1=8, Mod4=0x40).
    pub fn x11_mask(&self) -> u16 {
        let mut m = 0;
        if self.mods & MOD_SHIFT != 0 {
            m |= 0x01;
        }
        if self.mods & MOD_CTRL != 0 {
            m |= 0x04;
        }
        if self.mods & MOD_ALT != 0 {
            m |= 0x08;
        }
        if self.mods & MOD_SUPER != 0 {
            m |= 0x40;
        }
        m
    }
}

/// X11 keysym for a key name (Latin-1 chars map to themselves).
pub fn keysym(name: &str) -> Option<u32> {
    let n = name.to_lowercase();
    let named = match n.as_str() {
        "space" => Some(0x20),
        "return" | "enter" => Some(0xff0d),
        "tab" => Some(0xff09),
        "escape" | "esc" => Some(0xff1b),
        "backspace" => Some(0xff08),
        "grave" | "backtick" => Some(0x60),
        "comma" => Some(0x2c),
        "period" => Some(0x2e),
        "slash" => Some(0x2f),
        "semicolon" => Some(0x3b),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if let Some(rest) = n.strip_prefix('f') {
        if let Ok(k) = rest.parse::<u32>() {
            if (1..=12).contains(&k) {
                return Some(0xffbe + k - 1);
            }
        }
    }
    let mut chars = n.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c as u32),
        _ => None,
    }
}

const BIND_HELP: &str = "Bind the command `fudge toggle` to a custom keyboard shortcut in your desktop settings \
(on GNOME you can run `fudge install-shortcut`).";

/// Start the global hotkey listener. `on_trigger(timestamp)` is called from a background thread
/// (timestamp is the X server time of the key event, or 0 when unknown).
#[cfg(target_os = "linux")]
pub fn start(spec: &str, on_trigger: impl Fn(u32) + Send + 'static) {
    let hk = match Hotkey::parse(spec) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("invalid hotkey in config: {e}. {BIND_HELP}");
            return;
        }
    };
    if std::env::var("XDG_SESSION_TYPE").map(|t| t == "wayland").unwrap_or(false) {
        log::warn!("Wayland session: global hotkeys cannot be grabbed by applications. {BIND_HELP}");
        return;
    }
    if std::env::var_os("DISPLAY").is_none() {
        log::warn!("no X11 display: cannot grab {spec}. {BIND_HELP}");
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let spawned = std::thread::Builder::new().name("fudge-hotkey".into()).spawn(move || {
        x11::run(hk, tx, on_trigger);
    });
    if spawned.is_err() {
        log::warn!("could not start hotkey thread");
        return;
    }
    match rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(Ok(())) => log::info!("global hotkey {spec} grabbed"),
        Ok(Err(e)) => log::warn!("could not grab hotkey {spec}: {e}. Another program (or the desktop) owns it. {BIND_HELP}"),
        Err(_) => log::warn!("hotkey grab did not report back. {BIND_HELP}"),
    }
}

#[cfg(target_os = "linux")]
mod x11 {
    use super::*;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, GrabMode, ModMask};
    use x11rb::protocol::Event;

    pub fn run(hk: Hotkey, ready: std::sync::mpsc::Sender<Result<(), String>>, on_trigger: impl Fn(u32)) {
        let report = |r: Result<(), String>| {
            let _ = ready.send(r);
        };
        let (conn, screen_num) = match x11rb::connect(None) {
            Ok(c) => c,
            Err(e) => return report(Err(format!("cannot connect to X11: {e}"))),
        };
        let setup = conn.setup();
        let root = setup.roots[screen_num].root;
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let sym = keysym(&hk.key).unwrap_or(0);
        let map = match conn.get_keyboard_mapping(min, max - min + 1).map(|c| c.reply()) {
            Ok(Ok(m)) => m,
            _ => return report(Err("cannot read keyboard mapping".into())),
        };
        let per = map.keysyms_per_keycode as usize;
        let want = if (0x41..=0x5a).contains(&sym) { sym + 32 } else { sym };
        let keycode = map
            .keysyms
            .chunks(per.max(1))
            .position(|c| c.contains(&want))
            .map(|i| min + i as u8);
        let Some(keycode) = keycode else {
            return report(Err(format!("key {:?} not on this keyboard layout", hk.key)));
        };
        let base = hk.x11_mask();
        // Lock (Caps) = 0x02, Mod2 (NumLock) = 0x10.
        for extra in [0u16, 0x02, 0x10, 0x12] {
            let mask = ModMask::from(base | extra);
            let res = conn
                .grab_key(false, root, mask, keycode, GrabMode::ASYNC, GrabMode::ASYNC)
                .map_err(|e| e.to_string())
                .and_then(|c| c.check().map_err(|e| e.to_string()));
            if let Err(e) = res {
                return report(Err(e));
            }
        }
        let _ = conn.flush();
        report(Ok(()));
        loop {
            match conn.wait_for_event() {
                Ok(Event::KeyPress(ev)) => on_trigger(ev.time),
                Ok(_) => {}
                Err(e) => {
                    log::error!("X11 connection lost, hotkey disabled: {e}");
                    return;
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn start(spec: &str, on_trigger: impl Fn(u32) + Send + 'static) {
    // UNTESTED stub (cannot be built on Linux). global-hotkey must be created on the main thread
    // with a running event loop; GTK's loop on macOS satisfies that. We poll its channel.
    use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
    let Ok(hk) = spec.parse::<HotKey>() else {
        log::warn!("invalid hotkey {spec:?}");
        return;
    };
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => return log::warn!("hotkey manager: {e}"),
    };
    if let Err(e) = manager.register(hk) {
        return log::warn!("cannot register {spec}: {e}");
    }
    Box::leak(Box::new(manager));
    std::thread::spawn(move || loop {
        if let Ok(ev) = GlobalHotKeyEvent::receiver().recv() {
            if ev.state == HotKeyState::Pressed {
                on_trigger(0);
            }
        }
    });
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn start(_spec: &str, _on_trigger: impl Fn(u32) + Send + 'static) {
    log::warn!("global hotkeys are not supported on this platform. {BIND_HELP}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common() {
        assert_eq!(Hotkey::parse("Alt+space").unwrap(), Hotkey { mods: MOD_ALT, key: "space".into() });
        let h = Hotkey::parse("Ctrl+Alt+K").unwrap();
        assert_eq!(h.mods, MOD_CTRL | MOD_ALT);
        assert_eq!(h.key, "k");
        assert_eq!(Hotkey::parse("super+Space").unwrap().mods, MOD_SUPER);
        assert_eq!(Hotkey::parse("Ctrl + Shift + F5").unwrap().key, "f5");
    }

    #[test]
    fn rejects_bad() {
        assert!(Hotkey::parse("space").is_err());
        assert!(Hotkey::parse("Alt").is_err());
        assert!(Hotkey::parse("Alt+a+b").is_err());
        assert!(Hotkey::parse("Alt+bogus").is_err());
        assert!(Hotkey::parse("").is_err());
    }

    #[test]
    fn x11_and_gnome() {
        let h = Hotkey::parse("Ctrl+Alt+k").unwrap();
        assert_eq!(h.x11_mask(), 0x04 | 0x08);
        assert_eq!(h.to_gnome_accel(), "<Control><Alt>k");
        assert_eq!(Hotkey::parse("Alt+space").unwrap().to_gnome_accel(), "<Alt>space");
        assert_eq!(Hotkey::parse("Super+space").unwrap().x11_mask(), 0x40);
    }

    #[test]
    fn keysyms() {
        assert_eq!(keysym("space"), Some(0x20));
        assert_eq!(keysym("K"), Some(b'k' as u32));
        assert_eq!(keysym("F1"), Some(0xffbe));
        assert_eq!(keysym("f12"), Some(0xffc9));
        assert_eq!(keysym("f13"), None);
    }
}
