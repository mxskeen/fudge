//! Built-in providers for Fudge Launcher.

use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use fudge_core::{Config, Engine, History, Provider};
use fudge_platform::Platform;

pub mod apps;
pub mod compute;
pub mod files;
pub mod folders;
pub mod kill;
pub mod settings;
pub mod shell;
pub mod system;
pub mod websearch;
pub mod windows;

pub use fudge_core::{execute, execute_with, ExecOutcome};

/// All built-in providers. Background work (app scan, file index, rates) starts
/// immediately and bumps `epoch` when new data is ready.
pub fn default_providers(cfg: &Config, platform: Arc<dyn Platform>, epoch: Arc<AtomicU64>) -> Vec<Arc<dyn Provider>> {
    vec![
        Arc::new(compute::ComputeProvider::new(cfg.network_features, epoch.clone())),
        Arc::new(apps::AppsProvider::new(epoch.clone())),
        Arc::new(files::FilesProvider::new(cfg, epoch.clone())),
        Arc::new(system::SystemProvider::new()),
        Arc::new(kill::KillProvider::new()),
        Arc::new(windows::WindowsProvider::new(platform.clone())),
        Arc::new(websearch::WebSearchProvider::new(cfg)),
        Arc::new(shell::ShellProvider::new()),
        Arc::new(folders::QuickFoldersProvider::new()),
        Arc::new(settings::SettingsProvider::new(platform.desktop())),
    ]
}

/// Load config, open history, create providers: everything a front end needs.
pub fn build_engine() -> (Engine, Arc<dyn Platform>) {
    let config = Config::load();
    let history = match History::open_default() {
        Ok(h) => h,
        Err(e) => {
            log::warn!("history unavailable, using memory only: {e:#}");
            History::in_memory()
        }
    };
    let platform = fudge_platform::current();
    let epoch = Arc::new(AtomicU64::new(0));
    let providers = default_providers(&config, platform.clone(), epoch.clone());
    (Engine::with_epoch(providers, Arc::new(history), config, epoch), platform)
}

pub(crate) fn bump(epoch: &AtomicU64) {
    epoch.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Replace the home prefix with `~` for display.
pub(crate) fn tilde(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return if rest.as_os_str().is_empty() { "~".into() } else { format!("~/{}", rest.display()) };
        }
    }
    path.display().to_string()
}
