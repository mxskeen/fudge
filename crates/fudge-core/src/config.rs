//! User configuration (TOML). Loading never writes; only `save()` does.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    #[default]
    Solid,
    Frosted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hotkey: String,
    pub style: Style,
    pub network_features: bool,
    pub history_enabled: bool,
    pub max_results: usize,
    /// Folders walked (deep) by the file index.
    pub file_search_roots: Vec<PathBuf>,
    /// Also index the top level of the home folder (depth 1).
    pub file_search_home_shallow: bool,
    /// Directory names never entered or indexed.
    pub file_search_exclude: Vec<String>,
    /// keyword -> URL template with `{q}`.
    pub web_searches: BTreeMap<String, String>,
    /// Terminal command for `>` commands; auto-detected when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
}

pub fn default_file_search_roots() -> Vec<PathBuf> {
    [
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::picture_dir(),
        dirs::audio_dir(),
        dirs::video_dir(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn default_file_search_exclude() -> Vec<String> {
    [
        "node_modules", ".git", "target", ".cache", "__pycache__", ".venv", "venv", ".tox",
        ".gradle", ".cargo", ".rustup", ".npm", "build", "dist", ".next", "Library",
        "site-packages", ".Trash",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn default_web_searches() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let mut add = |k: &str, v: &str| {
        m.insert(k.to_string(), v.to_string());
    };
    add("/online", "https://duckduckgo.com/?q={q}");
    add("/g", "https://www.google.com/search?q={q}");
    add("/yt", "https://www.youtube.com/results?search_query={q}");
    add("/gh", "https://github.com/search?q={q}");
    add("/w", "https://en.wikipedia.org/w/index.php?search={q}");
    add("/maps", "https://www.openstreetmap.org/search?query={q}");
    add("/crates", "https://crates.io/search?q={q}");
    if cfg!(target_os = "macos") {
        add("/brew", "https://formulae.brew.sh/search?q={q}");
    } else {
        add("/aur", "https://aur.archlinux.org/packages?K={q}");
    }
    m
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+space".into(),
            style: Style::Solid,
            network_features: true,
            history_enabled: true,
            max_results: 8,
            file_search_roots: default_file_search_roots(),
            file_search_home_shallow: true,
            file_search_exclude: default_file_search_exclude(),
            web_searches: default_web_searches(),
            terminal: None,
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        crate::paths::config_dir().map(|d| d.join("config.toml"))
    }

    /// Load the user's config; missing file or malformed content gives defaults
    /// (malformed is logged). Never writes.
    pub fn load() -> Config {
        match Self::path() {
            Some(p) => Self::load_from(&p),
            None => Config::default(),
        }
    }

    pub fn load_from(path: &Path) -> Config {
        match std::fs::read_to_string(path) {
            Ok(text) => match Self::from_toml(&text) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("ignoring malformed config {}: {e:#}", path.display());
                    Config::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => {
                log::warn!("cannot read config {}: {e}", path.display());
                Config::default()
            }
        }
    }

    pub fn from_toml(text: &str) -> Result<Config> {
        toml::from_str(text).context("parsing config TOML")
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("serializing config")
    }

    pub fn save(&self) -> Result<()> {
        let p = Self::path().context("no config directory")?;
        self.save_to(&p)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_toml()?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let c = Config::default();
        assert_eq!(c.hotkey, "Alt+space");
        assert_eq!(c.style, Style::Solid);
        assert!(c.network_features && c.history_enabled);
        assert_eq!(c.max_results, 8);
        assert_eq!(c.web_searches["/online"], "https://duckduckgo.com/?q={q}");
        assert!(c.web_searches.contains_key("/yt"));
        assert!(c.file_search_exclude.iter().any(|s| s == "node_modules"));
        assert!(c.terminal.is_none());
    }

    #[test]
    fn toml_round_trip() {
        let c = Config {
            terminal: Some("kitty".into()),
            style: Style::Frosted,
            max_results: 5,
            ..Config::default()
        };
        let text = c.to_toml().unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), c);
    }

    #[test]
    fn partial_file_uses_defaults() {
        let c = Config::from_toml("max_results = 3\nstyle = \"frosted\"\n").unwrap();
        assert_eq!(c.max_results, 3);
        assert_eq!(c.style, Style::Frosted);
        assert_eq!(c.hotkey, "Alt+space");
    }

    #[test]
    fn malformed_gives_defaults_and_load_does_not_write() {
        let dir = std::env::temp_dir().join(format!("fudge-cfg-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("config.toml");
        std::fs::write(&bad, "max_results = [[[").unwrap();
        assert_eq!(Config::load_from(&bad), Config::default());
        let missing = dir.join("none.toml");
        assert_eq!(Config::load_from(&missing), Config::default());
        assert!(!missing.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
