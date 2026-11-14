//! Linux backend: gio/xdg-open, systemd, per-desktop commands, X11/Hyprland/Sway windows.

mod windows;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::util::{exec_to_argv, find_in_path, spawn_args, spawn_detached, spawn_first};
use crate::{kill_pids, DesktopKind, Platform, SessionKind, SystemCommand, WindowId, WindowInfo};

pub struct LinuxPlatform {
    desktop: DesktopKind,
    session: SessionKind,
}

pub(crate) fn desktop_from_env(xdg_current_desktop: &str) -> DesktopKind {
    let d = xdg_current_desktop.to_ascii_lowercase();
    let has = |s: &str| d.split(':').any(|p| p.contains(s));
    if has("hyprland") {
        DesktopKind::Hyprland
    } else if has("sway") {
        DesktopKind::Sway
    } else if has("gnome") || has("unity") || has("budgie") || has("cinnamon") || has("pantheon") {
        DesktopKind::Gnome
    } else if has("kde") || has("plasma") {
        DesktopKind::Kde
    } else if has("xfce") {
        DesktopKind::Xfce
    } else {
        DesktopKind::Other
    }
}

pub(crate) fn session_from_env(session_type: Option<&str>, wayland_display: bool) -> SessionKind {
    match session_type.map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "wayland" => SessionKind::Wayland,
        Some(s) if s == "x11" => SessionKind::X11,
        _ if wayland_display => SessionKind::Wayland,
        _ => SessionKind::X11,
    }
}

/// Extra args a terminal needs between its own flags and the command to run.
fn terminal_exec_args(bin: &str) -> &'static [&'static str] {
    match bin {
        "gnome-terminal" => &["--"],
        "wezterm" => &["start", "--"],
        "kitty" | "foot" | "footclient" => &[],
        _ => &["-e"],
    }
}

const TERMINALS: &[&str] = &[
    "x-terminal-emulator",
    "gnome-terminal",
    "kgx",
    "konsole",
    "alacritty",
    "kitty",
    "wezterm",
    "foot",
    "xterm",
];

/// Choose a terminal: explicit config, then $TERMINAL, then the first installed known one.
pub(crate) fn pick_terminal(
    config: Option<&str>,
    env_terminal: Option<&str>,
    exists: &dyn Fn(&str) -> bool,
) -> Option<String> {
    for cand in [config, env_terminal].into_iter().flatten() {
        let cand = cand.trim();
        if let Some(first) = cand.split_whitespace().next() {
            if exists(first) {
                return Some(cand.to_string());
            }
        }
    }
    TERMINALS.iter().find(|t| exists(t)).map(|t| t.to_string())
}

impl LinuxPlatform {
    pub fn new() -> Self {
        let desktop = desktop_from_env(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default());
        let session = session_from_env(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
        );
        Self { desktop, session }
    }
}

impl Default for LinuxPlatform {
    fn default() -> Self {
        Self::new()
    }
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = dirs::data_dir() {
        dirs.push(d.join("applications"));
    }
    let xdg = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in xdg.split(':').filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(d).join("applications"));
    }
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs.push("/var/lib/snapd/desktop/applications".into());
    dirs
}

fn find_desktop_file(id: &str) -> Option<PathBuf> {
    let id = if id.ends_with(".desktop") { id.to_string() } else { format!("{id}.desktop") };
    for d in desktop_dirs() {
        let p = d.join(&id);
        if p.is_file() {
            return Some(p);
        }
        // Ids for nested files use '-' for '/'.
        let nested = d.join(id.replacen('-', "/", 1));
        if nested.is_file() {
            return Some(nested);
        }
    }
    None
}

/// Fallback when `gtk-launch` is missing or fails: parse Exec ourselves.
fn launch_from_desktop_file(id: &str) -> Result<()> {
    let path = find_desktop_file(id).with_context(|| format!("no desktop file for {id}"))?;
    let text = std::fs::read_to_string(&path)?;
    let mut in_entry = false;
    let mut exec = None;
    let mut workdir = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry {
            if let Some(v) = line.strip_prefix("Exec=") {
                exec.get_or_insert_with(|| v.to_string());
            } else if let Some(v) = line.strip_prefix("Path=") {
                workdir = Some(v.to_string());
            }
        }
    }
    let exec = exec.with_context(|| format!("{} has no Exec line", path.display()))?;
    let argv = exec_to_argv(&exec);
    let (prog, args) = argv.split_first().context("empty Exec line")?;
    let mut cmd = Command::new(prog);
    cmd.args(args);
    if let Some(w) = workdir {
        if Path::new(&w).is_dir() {
            cmd.current_dir(w);
        }
    }
    spawn_detached(&mut cmd)
}

impl Platform for LinuxPlatform {
    fn open_path(&self, p: &Path) -> Result<()> {
        let p = p.to_string_lossy();
        spawn_first(&[&["gio", "open", &p], &["xdg-open", &p]])
    }

    fn reveal_in_folder(&self, p: &Path) -> Result<()> {
        // Ask the file manager to select the item; otherwise open the parent folder.
        let uri = format!("file://{}", p.to_string_lossy().replace(' ', "%20"));
        if find_in_path("dbus-send").is_some() {
            let arg = format!("array:string:{uri}");
            let ok = Command::new("dbus-send")
                .args([
                    "--session",
                    "--print-reply",
                    "--dest=org.freedesktop.FileManager1",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                    &arg,
                    "string:",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return Ok(());
            }
        }
        let parent = if p.is_dir() { p } else { p.parent().unwrap_or(p) };
        self.open_path(parent)
    }

    fn open_url(&self, u: &str) -> Result<()> {
        spawn_first(&[&["xdg-open", u], &["gio", "open", u]])
    }

    fn launch_app(&self, id: &str) -> Result<()> {
        if let Some(gtk_launch) = find_in_path("gtk-launch") {
            let mut cmd = Command::new(gtk_launch);
            cmd.arg(id)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
            let mut child = cmd.spawn().context("failed to start gtk-launch")?;
            let id = id.to_string();
            std::thread::spawn(move || {
                let ok = child.wait().map(|s| s.success()).unwrap_or(false);
                if !ok {
                    if let Err(e) = launch_from_desktop_file(&id) {
                        log::warn!("launching {id} failed: {e:#}");
                    }
                }
            });
            Ok(())
        } else {
            launch_from_desktop_file(id)
        }
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        windows::list(self.desktop, self.session)
    }

    fn focus_window(&self, id: &WindowId) -> Result<()> {
        windows::focus(self.desktop, self.session, id)
    }

    fn system(&self, c: SystemCommand) -> Result<()> {
        match c {
            SystemCommand::Lock => spawn_first(&[
                &["loginctl", "lock-session"],
                &["xdg-screensaver", "lock"],
            ]),
            SystemCommand::Sleep => spawn_args("systemctl", &["suspend"]),
            SystemCommand::Restart => spawn_args("systemctl", &["reboot"]),
            SystemCommand::Shutdown => spawn_args("systemctl", &["poweroff"]),
            SystemCommand::EmptyTrash => spawn_args("gio", &["trash", "--empty"]),
            SystemCommand::Logout => match self.desktop {
                DesktopKind::Gnome => {
                    spawn_args("gnome-session-quit", &["--logout", "--no-prompt"])
                }
                DesktopKind::Kde => spawn_first(&[
                    &["qdbus", "org.kde.Shutdown", "/Shutdown", "logout"],
                    &["qdbus6", "org.kde.Shutdown", "/Shutdown", "logout"],
                ]),
                DesktopKind::Hyprland => spawn_args("hyprctl", &["dispatch", "exit"]),
                DesktopKind::Sway => spawn_args("swaymsg", &["exit"]),
                _ => match std::env::var("XDG_SESSION_ID") {
                    Ok(id) if !id.is_empty() => spawn_args("loginctl", &["terminate-session", &id]),
                    _ => spawn_args("loginctl", &["terminate-user", &whoami()]),
                },
            },
        }
    }

    fn kill(&self, pids: &[u32]) -> Result<()> {
        kill_pids(pids)
    }

    fn run_shell(&self, cmd: &str, in_terminal: bool, terminal: Option<&str>) -> Result<()> {
        if !in_terminal {
            return spawn_args("sh", &["-c", cmd]);
        }
        let env_term = std::env::var("TERMINAL").ok();
        let term = pick_terminal(terminal, env_term.as_deref(), &|b| find_in_path(b).is_some())
            .context("no terminal emulator found; set `terminal` in config.toml")?;
        let mut parts = term.split_whitespace();
        let bin = parts.next().context("empty terminal setting")?;
        let extra: Vec<&str> = parts.collect();
        let base = Path::new(bin).file_name().and_then(|s| s.to_str()).unwrap_or(bin);
        // Keep the window open after the command finishes.
        let script = format!("{cmd}\nprintf '\\n[done] press Enter to close '; read _");
        let mut c = Command::new(bin);
        c.args(&extra).args(terminal_exec_args(base)).args(["sh", "-c", &script]);
        spawn_detached(&mut c)
    }

    fn open_settings_panel(&self, panel: &str) -> Result<()> {
        match self.desktop {
            DesktopKind::Kde => spawn_first(&[&["systemsettings", panel], &["systemsettings5", panel]]),
            DesktopKind::Xfce => spawn_first(&[
                &["gnome-control-center", panel],
                &["xfce4-settings-manager"],
            ]),
            _ => spawn_first(&[
                &["gnome-control-center", panel],
                &["systemsettings", panel],
                &["xfce4-settings-manager"],
            ]),
        }
    }

    fn desktop(&self) -> DesktopKind {
        self.desktop
    }

    fn session(&self) -> SessionKind {
        self.session
    }
}

fn whoami() -> String {
    std::env::var("USER").unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_detection() {
        assert_eq!(desktop_from_env("ubuntu:GNOME"), DesktopKind::Gnome);
        assert_eq!(desktop_from_env("KDE"), DesktopKind::Kde);
        assert_eq!(desktop_from_env("Hyprland"), DesktopKind::Hyprland);
        assert_eq!(desktop_from_env("sway"), DesktopKind::Sway);
        assert_eq!(desktop_from_env("XFCE"), DesktopKind::Xfce);
        assert_eq!(desktop_from_env(""), DesktopKind::Other);
    }

    #[test]
    fn session_detection() {
        assert_eq!(session_from_env(Some("wayland"), false), SessionKind::Wayland);
        assert_eq!(session_from_env(Some("x11"), true), SessionKind::X11);
        assert_eq!(session_from_env(None, true), SessionKind::Wayland);
        assert_eq!(session_from_env(None, false), SessionKind::X11);
    }

    #[test]
    fn terminal_choice() {
        let have = |n: &str| n == "kitty" || n == "xterm";
        assert_eq!(pick_terminal(Some("kitty"), None, &have).as_deref(), Some("kitty"));
        assert_eq!(pick_terminal(Some("nope"), Some("xterm"), &have).as_deref(), Some("xterm"));
        assert_eq!(pick_terminal(None, None, &have).as_deref(), Some("kitty"));
        assert_eq!(pick_terminal(None, None, &|_| false), None);
        assert_eq!(terminal_exec_args("gnome-terminal"), &["--"]);
        assert_eq!(terminal_exec_args("wezterm"), &["start", "--"]);
        assert_eq!(terminal_exec_args("konsole"), &["-e"]);
    }
}
