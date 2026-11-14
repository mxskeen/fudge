//! `win <name>` (and unprefixed) window switching. Window switches are never stored.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fudge_core::fuzzy;
use fudge_core::{Action, Icon, Platform, Provider, Query, ResultItem, WindowInfo};

type WindowCache = Option<(Instant, Result<Vec<WindowInfo>, String>)>;

const CACHE_TTL: Duration = Duration::from_millis(500);
const UNPREFIXED_FACTOR: f32 = 0.7;

pub struct WindowsProvider {
    platform: Arc<dyn Platform>,
    cache: Mutex<WindowCache>,
    keywords: Vec<&'static str>,
}

impl WindowsProvider {
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self { platform, cache: Mutex::new(None), keywords: vec!["win"] }
    }

    fn windows(&self) -> Result<Vec<WindowInfo>, String> {
        let mut c = self.cache.lock().unwrap();
        if let Some((t, v)) = c.as_ref() {
            if t.elapsed() < CACHE_TTL {
                return v.clone();
            }
        }
        let fresh = self.platform.list_windows().map_err(|e| format!("{e:#}"));
        *c = Some((Instant::now(), fresh.clone()));
        fresh
    }
}

fn to_item(w: &WindowInfo, score: f32) -> ResultItem {
    let title = if w.title.is_empty() { w.app_name.clone() } else { w.title.clone() };
    let icon = w
        .app_id
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| (!w.app_name.is_empty()).then(|| w.app_name.to_lowercase()))
        .map(Icon::Named)
        .unwrap_or(Icon::None);
    ResultItem::new(format!("win:{}", w.id.0), title, "windows")
        .subtitle(if w.app_name.is_empty() { "Window".to_string() } else { format!("{} · Window", w.app_name) })
        .icon(icon)
        .score(score)
        .action(Action::FocusWindow(w.id.clone()))
        .no_history()
}

impl Provider for WindowsProvider {
    fn id(&self) -> &'static str {
        "windows"
    }

    fn keywords(&self) -> &[&'static str] {
        &self.keywords
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        let explicit = q.keyword.as_deref() == Some("win");
        if q.keyword.is_some() && !explicit {
            return Vec::new();
        }
        let text = if explicit { q.rest.as_str() } else { q.text.as_str() };
        if !explicit && text.chars().count() < 3 {
            return Vec::new();
        }
        let wins = match self.windows() {
            Ok(w) => w,
            Err(e) if explicit => {
                return vec![ResultItem::new("win:unavailable", "Window switching unavailable", "windows")
                    .subtitle(e.clone())
                    .icon(Icon::Named("dialog-information".into()))
                    .score(1.0)
                    .action(Action::Copy(e))
                    .no_history()];
            }
            Err(_) => return Vec::new(),
        };
        let mut out: Vec<ResultItem> = wins
            .iter()
            .filter_map(|w| {
                if text.is_empty() {
                    return Some(to_item(w, 50.0));
                }
                let s = fuzzy::best(text, [w.title.as_str(), w.app_name.as_str()])?;
                if !explicit && s < 45.0 {
                    return None; // keep scattered fuzzy noise out of ordinary searches
                }
                Some(to_item(w, if explicit { s } else { s * UNPREFIXED_FACTOR }))
            })
            .collect();
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(10);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fudge_core::{DesktopKind, SessionKind, SystemCommand, WindowId};
    use std::path::Path;

    struct P(Result<Vec<WindowInfo>, String>);
    impl Platform for P {
        fn open_path(&self, _: &Path) -> anyhow::Result<()> { Ok(()) }
        fn reveal_in_folder(&self, _: &Path) -> anyhow::Result<()> { Ok(()) }
        fn open_url(&self, _: &str) -> anyhow::Result<()> { Ok(()) }
        fn launch_app(&self, _: &str) -> anyhow::Result<()> { Ok(()) }
        fn list_windows(&self) -> anyhow::Result<Vec<WindowInfo>> { self.0.clone().map_err(|e| anyhow::anyhow!(e)) }
        fn focus_window(&self, _: &WindowId) -> anyhow::Result<()> { Ok(()) }
        fn system(&self, _: SystemCommand) -> anyhow::Result<()> { Ok(()) }
        fn kill(&self, _: &[u32]) -> anyhow::Result<()> { Ok(()) }
        fn run_shell(&self, _: &str, _: bool, _: Option<&str>) -> anyhow::Result<()> { Ok(()) }
        fn open_settings_panel(&self, _: &str) -> anyhow::Result<()> { Ok(()) }
        fn desktop(&self) -> DesktopKind { DesktopKind::Other }
        fn session(&self) -> SessionKind { SessionKind::X11 }
    }

    fn win(id: &str, title: &str, app: &str) -> WindowInfo {
        WindowInfo { id: WindowId(id.into()), title: title.into(), app_name: app.into(), app_id: None, pid: None }
    }

    fn prov(w: Vec<WindowInfo>) -> WindowsProvider {
        WindowsProvider::new(Arc::new(P(Ok(w))))
    }

    fn kw(s: &str) -> bool {
        s == "win"
    }

    #[test]
    fn keyword_and_unprefixed() {
        let p = prov(vec![win("1", "Design - Figma", "Figma"), win("2", "Terminal", "foot")]);
        let r = p.query(&Query::parse("win figma", kw));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].actions, vec![Action::FocusWindow(WindowId("1".into()))]);
        assert!(!r[0].record_history);
        let u = p.query(&Query::parse("figma", kw));
        assert_eq!(u.len(), 1);
        assert!(u[0].score < r[0].score);
        assert!(p.query(&Query::parse("fi", kw)).is_empty());
        assert_eq!(p.query(&Query::parse("win", kw)).len(), 2);
    }

    #[test]
    fn errors_only_surface_with_keyword() {
        let p = WindowsProvider::new(Arc::new(P(Err("needs extension".into()))));
        assert_eq!(p.query(&Query::parse("win x", kw)).len(), 1);
        assert!(p.query(&Query::parse("figma", kw)).is_empty());
    }
}
