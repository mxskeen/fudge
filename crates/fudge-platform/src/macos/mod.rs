//! macOS backend: `open`, `pmset`, osascript.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::util::{spawn_args, spawn_detached};
use crate::{kill_pids, DesktopKind, Platform, SessionKind, SystemCommand, WindowId, WindowInfo};

pub struct MacPlatform;

impl MacPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacPlatform {
    fn default() -> Self {
        Self::new()
    }
}

fn osascript(script: &str) -> Result<String> {
    let out = Command::new("osascript")
        .args(["-e", script])
        .output()
        .context("failed to run osascript")?;
    if !out.status.success() {
        bail!("osascript: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn osascript_detached(script: &str) -> Result<()> {
    spawn_args("osascript", &["-e", script])
}

// TODO: replace with CGWindowListCopyWindowInfo (needs objc bindings) for speed.
const LIST_JXA: &str = r#"
var se = Application('System Events');
var out = [];
se.processes.whose({backgroundOnly: false, visible: true})().forEach(function (p) {
  try {
    var pid = p.unixId();
    p.windows().forEach(function (w, i) {
      try {
        out.push({id: pid + ':' + i, title: w.name() || '', app: p.name(), bundle: p.bundleIdentifier(), pid: pid});
      } catch (e) {}
    });
  } catch (e) {}
});
JSON.stringify(out);
"#;

pub(crate) fn parse_window_json(json: &str) -> Result<Vec<WindowInfo>> {
    let v: Value = serde_json::from_str(json.trim()).context("bad window list")?;
    let mut out = Vec::new();
    for w in v.as_array().context("expected array")? {
        let s = |k: &str| w.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let Some(id) = w.get("id").and_then(Value::as_str) else { continue };
        out.push(WindowInfo {
            id: WindowId(id.to_string()),
            title: s("title"),
            app_name: s("app"),
            app_id: w.get("bundle").and_then(Value::as_str).map(str::to_string),
            pid: w.get("pid").and_then(Value::as_u64).map(|p| p as u32),
        });
    }
    Ok(out)
}

impl Platform for MacPlatform {
    fn open_path(&self, p: &Path) -> Result<()> {
        spawn_args("open", &[&p.to_string_lossy()])
    }

    fn reveal_in_folder(&self, p: &Path) -> Result<()> {
        spawn_args("open", &["-R", &p.to_string_lossy()])
    }

    fn open_url(&self, u: &str) -> Result<()> {
        spawn_args("open", &[u])
    }

    fn launch_app(&self, id: &str) -> Result<()> {
        if id.contains('/') {
            spawn_args("open", &[id])
        } else {
            spawn_args("open", &["-a", id])
        }
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        let out = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", LIST_JXA])
            .output()
            .context("failed to run osascript")?;
        if !out.status.success() {
            bail!(
                "listing windows failed (grant Accessibility/Automation permission?): {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        // JXA prints its result on stderr or stdout depending on version.
        let text = if out.stdout.iter().any(|b| !b.is_ascii_whitespace()) {
            String::from_utf8_lossy(&out.stdout).into_owned()
        } else {
            String::from_utf8_lossy(&out.stderr).into_owned()
        };
        parse_window_json(&text)
    }

    fn focus_window(&self, id: &WindowId) -> Result<()> {
        let (pid, idx) = id.0.split_once(':').context("bad window id")?;
        let pid: u32 = pid.parse().context("bad pid")?;
        let idx: u32 = idx.parse().context("bad window index")?;
        let script = format!(
            "tell application \"System Events\"\n\
             set p to first process whose unix id is {pid}\n\
             set frontmost of p to true\n\
             try\n perform action \"AXRaise\" of window {n} of p\n end try\n\
             end tell",
            n = idx + 1
        );
        osascript_detached(&script)
    }

    fn system(&self, c: SystemCommand) -> Result<()> {
        match c {
            SystemCommand::Lock => spawn_args("pmset", &["displaysleepnow"]),
            SystemCommand::Sleep => spawn_args("pmset", &["sleepnow"]),
            SystemCommand::Restart => {
                osascript_detached("tell application \"System Events\" to restart")
            }
            SystemCommand::Shutdown => {
                osascript_detached("tell application \"System Events\" to shut down")
            }
            SystemCommand::Logout => {
                osascript_detached("tell application \"System Events\" to log out")
            }
            SystemCommand::EmptyTrash => {
                osascript_detached("tell application \"Finder\" to empty trash")
            }
        }
    }

    fn kill(&self, pids: &[u32]) -> Result<()> {
        kill_pids(pids)
    }

    fn run_shell(&self, cmd: &str, in_terminal: bool, terminal: Option<&str>) -> Result<()> {
        if !in_terminal {
            return spawn_args("sh", &["-c", cmd]);
        }
        let script = format!("{cmd}\nprintf '\\n[done] press Enter to close '; read _");
        let app = terminal.unwrap_or("Terminal");
        // Terminal.app / iTerm via a temp script is the most robust route.
        let path = std::env::temp_dir().join(format!("fudge-run-{}.command", std::process::id()));
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n"))?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        spawn_detached(Command::new("open").args(["-a", app]).arg(&path))
    }

    fn open_settings_panel(&self, panel: &str) -> Result<()> {
        spawn_args("open", &[panel])
    }

    fn desktop(&self) -> DesktopKind {
        DesktopKind::MacOs
    }

    fn session(&self) -> SessionKind {
        SessionKind::MacOs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_window_json() {
        let w = parse_window_json(r#"[{"id":"12:0","title":"Doc","app":"Pages","bundle":"com.apple.iWork.Pages","pid":12}]"#).unwrap();
        assert_eq!(w[0].app_name, "Pages");
        assert_eq!(w[0].pid, Some(12));
    }
}
