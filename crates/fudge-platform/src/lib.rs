//! OS glue behind one trait. This crate must not depend on `fudge-core`.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

mod util;
pub use util::{exec_to_argv, find_in_path};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemCommand {
    Lock,
    Sleep,
    Restart,
    Shutdown,
    Logout,
    EmptyTrash,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub app_name: String,
    pub app_id: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopKind {
    Gnome,
    Kde,
    Hyprland,
    Sway,
    Xfce,
    Other,
    MacOs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionKind {
    X11,
    Wayland,
    MacOs,
}

pub trait Platform: Send + Sync {
    fn open_path(&self, p: &Path) -> Result<()>;
    fn reveal_in_folder(&self, p: &Path) -> Result<()>;
    fn open_url(&self, u: &str) -> Result<()>;
    /// `id` is a desktop-file id on Linux ("firefox.desktop") and a bundle path on macOS.
    fn launch_app(&self, id: &str) -> Result<()>;
    fn list_windows(&self) -> Result<Vec<WindowInfo>>;
    fn focus_window(&self, id: &WindowId) -> Result<()>;
    fn system(&self, c: SystemCommand) -> Result<()>;
    fn kill(&self, pids: &[u32]) -> Result<()>;
    fn run_shell(&self, cmd: &str, in_terminal: bool, terminal: Option<&str>) -> Result<()>;
    fn open_settings_panel(&self, panel: &str) -> Result<()>;
    fn desktop(&self) -> DesktopKind;
    fn session(&self) -> SessionKind;
}

/// The platform backend for the running OS.
pub fn current() -> Arc<dyn Platform> {
    #[cfg(target_os = "linux")]
    {
        Arc::new(linux::LinuxPlatform::new())
    }
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacPlatform::new())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        compile_error!("fudge-platform supports Linux and macOS only")
    }
}

/// Send SIGTERM now, SIGKILL after 1.5 s to anything still alive (background thread).
pub(crate) fn kill_pids(pids: &[u32]) -> Result<()> {
    let mut failed = Vec::new();
    for &pid in pids {
        // SAFETY: plain syscall with integer arguments.
        let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        if rc != 0 {
            failed.push(pid);
        }
    }
    let alive: Vec<u32> = pids.iter().copied().filter(|p| !failed.contains(p)).collect();
    if !alive.is_empty() {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            for pid in alive {
                // SAFETY: signal 0 only probes for existence.
                let exists = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
                if exists {
                    // SAFETY: plain syscall.
                    unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
                }
            }
        });
    }
    if failed.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("could not signal pid(s) {:?} (permission denied or already gone)", failed)
    }
}
