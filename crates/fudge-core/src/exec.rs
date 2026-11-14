//! Runs an `Action` through the platform, records history, audit-logs risky actions.

use anyhow::Result;
use fudge_platform::Platform;

use crate::action::Action;
use crate::history::History;
use crate::item::ResultItem;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecOutcome {
    /// Whether the launcher window should hide now.
    pub hide_window: bool,
    /// Text the GUI must put on the clipboard (copying is done by the GUI).
    pub copied: Option<String>,
    /// Optional short status message to show.
    pub message: Option<String>,
}

pub fn execute(action: &Action, item: &ResultItem, platform: &dyn Platform, history: &History) -> Result<ExecOutcome> {
    execute_with(action, item, platform, history, None)
}

/// `terminal` is `Config::terminal`, used for `RunShell { in_terminal: true }`.
pub fn execute_with(
    action: &Action,
    item: &ResultItem,
    platform: &dyn Platform,
    history: &History,
    terminal: Option<&str>,
) -> Result<ExecOutcome> {
    let mut out = ExecOutcome { hide_window: true, ..Default::default() };

    // Audit before running so even a failing command is on record.
    match action {
        Action::RunShell { command, in_terminal } => {
            let _ = history.audit("shell", &format!("{} {command}", if *in_terminal { "[terminal]" } else { "[background]" }));
        }
        Action::KillProcesses { pids, name, protected } if *protected => {
            let _ = history.audit("kill", &format!("{name} pids={pids:?} (protected)"));
        }
        Action::System(c) if action.needs_confirm() => {
            let _ = history.audit("system", &format!("{c:?}"));
        }
        _ => {}
    }

    match action {
        Action::Open(p) => platform.open_path(p)?,
        Action::RevealInFolder(p) => platform.reveal_in_folder(p)?,
        Action::LaunchApp { id } => platform.launch_app(id)?,
        Action::OpenUrl(u) => platform.open_url(u)?,
        Action::Copy(text) => {
            out.copied = Some(text.clone());
        }
        Action::RunShell { command, in_terminal } => platform.run_shell(command, *in_terminal, terminal)?,
        Action::KillProcesses { pids, name, .. } => {
            platform.kill(pids)?;
            out.message = Some(format!("Asked {name} to quit"));
        }
        Action::System(c) => platform.system(*c)?,
        Action::FocusWindow(id) => platform.focus_window(id)?,
        Action::OpenSettingsPanel(p) => platform.open_settings_panel(p)?,
        Action::Plugin { plugin_id, .. } => {
            anyhow::bail!("plugin '{plugin_id}' cannot run: plugins are not supported yet")
        }
    }

    if let Err(e) = history.record(item) {
        log::warn!("recording history failed: {e:#}");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fudge_platform::{DesktopKind, SessionKind, SystemCommand, WindowId, WindowInfo};
    use std::path::Path;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Fake(Mutex<Vec<String>>);
    impl Fake {
        fn log(&self, s: String) -> Result<()> {
            self.0.lock().unwrap().push(s);
            Ok(())
        }
    }
    impl Platform for Fake {
        fn open_path(&self, p: &Path) -> Result<()> { self.log(format!("open {}", p.display())) }
        fn reveal_in_folder(&self, p: &Path) -> Result<()> { self.log(format!("reveal {}", p.display())) }
        fn open_url(&self, u: &str) -> Result<()> { self.log(format!("url {u}")) }
        fn launch_app(&self, id: &str) -> Result<()> { self.log(format!("launch {id}")) }
        fn list_windows(&self) -> Result<Vec<WindowInfo>> { Ok(vec![]) }
        fn focus_window(&self, id: &WindowId) -> Result<()> { self.log(format!("focus {}", id.0)) }
        fn system(&self, c: SystemCommand) -> Result<()> { self.log(format!("system {c:?}")) }
        fn kill(&self, pids: &[u32]) -> Result<()> { self.log(format!("kill {pids:?}")) }
        fn run_shell(&self, cmd: &str, t: bool, _: Option<&str>) -> Result<()> { self.log(format!("sh {t} {cmd}")) }
        fn open_settings_panel(&self, p: &str) -> Result<()> { self.log(format!("settings {p}")) }
        fn desktop(&self) -> DesktopKind { DesktopKind::Other }
        fn session(&self) -> SessionKind { SessionKind::X11 }
    }

    #[test]
    fn copy_returns_text_and_records() {
        let h = History::in_memory();
        let p = Fake::default();
        let item = ResultItem::new("c:1", "= 216", "compute").action(Action::Copy("216".into()));
        let o = execute(&item.actions[0], &item, &p, &h).unwrap();
        assert_eq!(o.copied.as_deref(), Some("216"));
        assert!(o.hide_window);
        assert_eq!(h.recents(5).len(), 1);
    }

    #[test]
    fn shell_is_audited_and_destructive_too() {
        let h = History::in_memory();
        let p = Fake::default();
        let a = Action::RunShell { command: "htop".into(), in_terminal: true };
        let item = ResultItem::new("sh:htop", "htop", "shell").action(a.clone());
        execute(&a, &item, &p, &h).unwrap();
        let s = Action::System(SystemCommand::Shutdown);
        execute(&s, &item, &p, &h).unwrap();
        let lock = Action::System(SystemCommand::Lock);
        execute(&lock, &item, &p, &h).unwrap();
        let log = h.audit_entries(10);
        assert_eq!(log.len(), 2);
        assert_eq!(p.0.lock().unwrap()[0], "sh true htop");
    }

    #[test]
    fn no_history_when_flag_off() {
        let h = History::in_memory();
        let p = Fake::default();
        let item = ResultItem::new("w:1", "win", "windows").action(Action::FocusWindow(WindowId("1".into()))).no_history();
        execute(&item.actions[0], &item, &p, &h).unwrap();
        assert!(h.recents(5).is_empty());
    }
}
