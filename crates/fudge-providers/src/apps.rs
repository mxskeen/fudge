//! Installed applications: XDG .desktop files on Linux, .app bundles on macOS.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

use fudge_core::fuzzy;
use fudge_core::{Action, Icon, Provider, Query, ResultItem};

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    /// Desktop-file id ("firefox.desktop") or bundle path on macOS.
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub keywords: Vec<String>,
    pub icon: Option<String>,
    /// Basename of the executable in `Exec=`.
    pub exec_name: Option<String>,
    pub path: PathBuf,
}

/// Locale preferences derived from `$LANG`, most specific first: "de_DE", "de".
pub fn lang_prefs(lang: &str) -> Vec<String> {
    let base = lang.split(['.', '@']).next().unwrap_or("");
    if base.is_empty() || base == "C" || base == "POSIX" {
        return Vec::new();
    }
    let mut v = vec![base.to_string()];
    if let Some((l, _)) = base.split_once('_') {
        v.push(l.to_string());
    }
    v
}

pub fn desktops_from_env(xdg_current_desktop: &str) -> Vec<String> {
    xdg_current_desktop
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('s') => out.push(' '),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(o) => {
                    out.push('\\');
                    out.push(o);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse the `[Desktop Entry]` group of a .desktop file. Returns `None` for entries
/// that must not be shown (NoDisplay, Hidden, wrong Type, filtered by desktop).
pub fn parse_desktop_entry(id: &str, path: &Path, content: &str, langs: &[String], current: &[String]) -> Option<App> {
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut localized: Vec<Option<String>> = vec![None; langs.len()];
    let mut generic: Option<String> = None;
    let mut generic_loc: Vec<Option<String>> = vec![None; langs.len()];
    let mut keywords: Option<String> = None;
    let mut kw_loc: Vec<Option<String>> = vec![None; langs.len()];
    let mut icon = None;
    let mut exec = None;
    let mut no_display = false;
    let mut hidden = false;
    let mut typ: Option<String> = None;
    let mut only_show_in: Vec<String> = Vec::new();
    let mut not_show_in: Vec<String> = Vec::new();

    let list = |v: &str| -> Vec<String> {
        v.split(';').map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect()
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if in_entry {
                break; // only the first group matters; actions come after
            }
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "Name" => name = Some(unescape(v)),
            "GenericName" => generic = Some(unescape(v)),
            "Keywords" => keywords = Some(unescape(v)),
            "Icon" => icon = Some(v.to_string()),
            "Exec" => exec = Some(v.to_string()),
            "NoDisplay" => no_display = v.eq_ignore_ascii_case("true"),
            "Hidden" => hidden = v.eq_ignore_ascii_case("true"),
            "Type" => typ = Some(v.to_string()),
            "OnlyShowIn" => only_show_in = list(v),
            "NotShowIn" => not_show_in = list(v),
            _ => {
                if let Some(loc) = k.strip_prefix("Name[").and_then(|r| r.strip_suffix(']')) {
                    if let Some(i) = langs.iter().position(|l| l == loc) {
                        localized[i] = Some(unescape(v));
                    }
                } else if let Some(loc) = k.strip_prefix("GenericName[").and_then(|r| r.strip_suffix(']')) {
                    if let Some(i) = langs.iter().position(|l| l == loc) {
                        generic_loc[i] = Some(unescape(v));
                    }
                } else if let Some(loc) = k.strip_prefix("Keywords[").and_then(|r| r.strip_suffix(']')) {
                    if let Some(i) = langs.iter().position(|l| l == loc) {
                        kw_loc[i] = Some(unescape(v));
                    }
                }
            }
        }
    }

    if no_display || hidden {
        return None;
    }
    if typ.as_deref() != Some("Application") {
        return None;
    }
    if !current.is_empty() {
        if !only_show_in.is_empty() && !only_show_in.iter().any(|d| current.contains(d)) {
            return None;
        }
        if not_show_in.iter().any(|d| current.contains(d)) {
            return None;
        }
    }
    let name = localized.into_iter().flatten().next().or(name)?;
    if name.is_empty() {
        return None;
    }
    let generic_name = generic_loc.into_iter().flatten().next().or(generic).filter(|s| !s.is_empty());
    let mut kws: Vec<String> = Vec::new();
    for src in kw_loc.into_iter().flatten().next().into_iter().chain(keywords) {
        kws.extend(src.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
    }
    let exec_name = exec.as_deref().and_then(|e| {
        let argv = fudge_platform::exec_to_argv(e);
        // Skip `env VAR=x` prefixes.
        let mut it = argv.iter();
        let mut first = it.next()?;
        if Path::new(first).file_name().and_then(|s| s.to_str()) == Some("env") {
            first = it.find(|a| !a.contains('='))?;
        }
        Path::new(first).file_name().and_then(|s| s.to_str()).map(str::to_string)
    });
    Some(App {
        id: id.to_string(),
        name,
        generic_name,
        keywords: kws,
        icon: icon.filter(|s| !s.is_empty()),
        exec_name,
        path: path.to_path_buf(),
    })
}

#[cfg(target_os = "linux")]
fn application_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let home_share = dirs::data_dir();
    if let Some(d) = &home_share {
        dirs.push(d.join("applications"));
        dirs.push(d.join("flatpak/exports/share/applications"));
    }
    let xdg = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
    let xdg = if xdg.is_empty() { "/usr/local/share:/usr/share".to_string() } else { xdg };
    for d in xdg.split(':').filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(d).join("applications"));
    }
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs.push("/var/lib/snapd/desktop/applications".into());
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

#[cfg(target_os = "linux")]
fn scan() -> Vec<App> {
    use std::collections::HashSet;
    let langs = lang_prefs(&std::env::var("LANG").unwrap_or_default());
    let current = desktops_from_env(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default());
    let mut seen: HashSet<String> = HashSet::new();
    let mut apps = Vec::new();
    for dir in application_dirs() {
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir).follow_links(true).max_depth(4).into_iter().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") || !entry.file_type().is_file() {
                continue;
            }
            let Ok(rel) = path.strip_prefix(&dir) else { continue };
            let id = rel.to_string_lossy().replace('/', "-");
            // Earlier directories win, even if the winner is hidden (that is how users hide apps).
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(path) else { continue };
            if let Some(app) = parse_desktop_entry(&id, path, &content, &langs, &current) {
                apps.push(app);
            }
        }
    }
    apps
}

#[cfg(target_os = "macos")]
fn scan() -> Vec<App> {
    let mut dirs: Vec<PathBuf> = vec!["/Applications".into()];
    if let Some(h) = dirs::home_dir() {
        dirs.push(h.join("Applications"));
    }
    dirs.push("/System/Applications".into());
    dirs.push("/System/Applications/Utilities".into());
    dirs.push("/Applications/Utilities".into());
    let mut seen = std::collections::HashSet::new();
    let mut apps = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            let Some(name) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            if !seen.insert(name.clone()) {
                continue;
            }
            apps.push(App {
                id: p.to_string_lossy().into_owned(),
                name,
                generic_name: None,
                keywords: Vec::new(),
                icon: None, // TODO: render the bundle's .icns
                exec_name: None,
                path: p,
            });
        }
    }
    apps
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn scan() -> Vec<App> {
    Vec::new()
}

/// Score an app against a (non-empty) query.
pub fn score_app(text: &str, app: &App) -> Option<f32> {
    let mut best = fuzzy::score(text, &app.name);
    let mut consider = |s: Option<f32>, factor: f32, min: f32| {
        if let Some(s) = s {
            if s >= min {
                let s = s * factor;
                best = Some(best.map_or(s, |b| b.max(s)));
            }
        }
    };
    if let Some(g) = &app.generic_name {
        consider(fuzzy::score(text, g), 0.8, 40.0);
    }
    for k in &app.keywords {
        consider(fuzzy::score(text, k), 0.7, 50.0);
    }
    if let Some(e) = &app.exec_name {
        consider(fuzzy::score(text, e), 0.75, 40.0);
    }
    best
}

pub struct AppsProvider {
    apps: Arc<RwLock<Vec<App>>>,
    epoch: Arc<AtomicU64>,
}

impl AppsProvider {
    /// Starts the first scan on a background thread.
    pub fn new(epoch: Arc<AtomicU64>) -> Self {
        let p = Self { apps: Arc::new(RwLock::new(Vec::new())), epoch };
        p.refresh();
        p
    }

    /// Provider pre-filled for tests.
    pub fn with_apps(apps: Vec<App>) -> Self {
        Self { apps: Arc::new(RwLock::new(apps)), epoch: Arc::new(AtomicU64::new(0)) }
    }
}

impl Provider for AppsProvider {
    fn id(&self) -> &'static str {
        "apps"
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        if q.keyword.is_some() || q.text.is_empty() {
            return Vec::new();
        }
        let apps = self.apps.read().unwrap();
        let mut hits: Vec<(f32, &App)> = apps.iter().filter_map(|a| score_app(&q.text, a).map(|s| (s, a))).collect();
        hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(12);
        hits.into_iter()
            .map(|(s, a)| {
                let icon = match &a.icon {
                    Some(i) if i.starts_with('/') => Icon::Path(PathBuf::from(i)),
                    Some(i) => Icon::Named(i.clone()),
                    None => Icon::Named("application-x-executable".into()),
                };
                let mut item = ResultItem::new(format!("app:{}", a.id), a.name.clone(), "apps")
                    .subtitle(a.generic_name.clone().unwrap_or_else(|| "App".into()))
                    .icon(icon)
                    .score(s)
                    .action(Action::LaunchApp { id: a.id.clone() });
                if cfg!(target_os = "macos") {
                    item = item.action(Action::RevealInFolder(a.path.clone()));
                }
                item
            })
            .collect()
    }

    fn refresh(&self) {
        let apps = self.apps.clone();
        let epoch = self.epoch.clone();
        std::thread::Builder::new()
            .name("fudge-apps-scan".into())
            .spawn(move || {
                let found = scan();
                log::debug!("apps: indexed {}", found.len());
                *apps.write().unwrap() = found;
                crate::bump(&epoch);
            })
            .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "\
[Desktop Entry]
Version=1.0
Type=Application
Name=Firefox Web Browser
Name[de]=Firefox Webbrowser
Name[de_AT]=Firefox Browser AT
GenericName=Web Browser
Keywords=internet;www;browser;
Icon=firefox
Exec=/usr/lib/firefox/firefox %u
Categories=Network;

[Desktop Action new-window]
Name=New Window
Exec=firefox --new-window %u
";

    fn p() -> PathBuf {
        PathBuf::from("/usr/share/applications/firefox.desktop")
    }

    #[test]
    fn parses_basic_fields() {
        let a = parse_desktop_entry("firefox.desktop", &p(), FIREFOX, &[], &[]).unwrap();
        assert_eq!(a.name, "Firefox Web Browser");
        assert_eq!(a.generic_name.as_deref(), Some("Web Browser"));
        assert_eq!(a.keywords, vec!["internet", "www", "browser"]);
        assert_eq!(a.icon.as_deref(), Some("firefox"));
        assert_eq!(a.exec_name.as_deref(), Some("firefox"));
    }

    #[test]
    fn localized_name() {
        let langs = lang_prefs("de_DE.UTF-8");
        assert_eq!(langs, vec!["de_DE", "de"]);
        let a = parse_desktop_entry("f", &p(), FIREFOX, &langs, &[]).unwrap();
        assert_eq!(a.name, "Firefox Webbrowser");
        let langs = lang_prefs("de_AT");
        let a = parse_desktop_entry("f", &p(), FIREFOX, &langs, &[]).unwrap();
        assert_eq!(a.name, "Firefox Browser AT");
        assert!(lang_prefs("C").is_empty());
    }

    #[test]
    fn hidden_entries_are_skipped() {
        let nd = FIREFOX.replace("Type=Application", "Type=Application\nNoDisplay=true");
        assert!(parse_desktop_entry("f", &p(), &nd, &[], &[]).is_none());
        let h = FIREFOX.replace("Type=Application", "Type=Application\nHidden=true");
        assert!(parse_desktop_entry("f", &p(), &h, &[], &[]).is_none());
        let link = FIREFOX.replace("Type=Application", "Type=Link");
        assert!(parse_desktop_entry("f", &p(), &link, &[], &[]).is_none());
    }

    #[test]
    fn only_and_not_show_in() {
        let gnome = desktops_from_env("ubuntu:GNOME");
        let only_kde = FIREFOX.replace("Type=Application", "Type=Application\nOnlyShowIn=KDE;");
        assert!(parse_desktop_entry("f", &p(), &only_kde, &[], &gnome).is_none());
        assert!(parse_desktop_entry("f", &p(), &only_kde, &[], &desktops_from_env("KDE")).is_some());
        let not_gnome = FIREFOX.replace("Type=Application", "Type=Application\nNotShowIn=GNOME;");
        assert!(parse_desktop_entry("f", &p(), &not_gnome, &[], &gnome).is_none());
    }

    #[test]
    fn exec_with_env_and_quotes() {
        let c = "[Desktop Entry]\nType=Application\nName=Code\nExec=env GDK_BACKEND=x11 \"/opt/Visual Studio Code/code\" --unity-launch %F\n";
        let a = parse_desktop_entry("code.desktop", &p(), c, &[], &[]).unwrap();
        assert_eq!(a.exec_name.as_deref(), Some("code"));
    }

    fn app(id: &str, name: &str) -> App {
        App { id: id.into(), name: name.into(), generic_name: None, keywords: vec![], icon: None, exec_name: None, path: PathBuf::new() }
    }

    #[test]
    fn query_ranks_acronym() {
        let prov = AppsProvider::with_apps(vec![
            app("a.desktop", "Evince Scanner Console"),
            app("code.desktop", "Visual Studio Code"),
            app("f.desktop", "Files"),
        ]);
        let r = prov.query(&Query::parse("vsc", |_| false));
        assert_eq!(r[0].id, "app:code.desktop");
        assert_eq!(r[0].actions[0], Action::LaunchApp { id: "code.desktop".into() });
    }

    #[test]
    fn keyword_and_generic_name_match() {
        let mut ff = app("firefox.desktop", "Firefox");
        ff.generic_name = Some("Web Browser".into());
        ff.keywords = vec!["internet".into()];
        let prov = AppsProvider::with_apps(vec![ff]);
        assert_eq!(prov.query(&Query::parse("internet", |_| false)).len(), 1);
        assert_eq!(prov.query(&Query::parse("web browser", |_| false)).len(), 1);
        assert!(prov.query(&Query::parse("kill firefox", |k| k == "kill")).is_empty());
    }
}
