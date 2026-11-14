//! File-name search over an in-memory index kept current by a filesystem watcher.
//!
//! Only names, kinds and modification times are indexed, never contents.
//! TODO: content search through Tracker/LocalSearch (GNOME) and Baloo (KDE) over D-Bus.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fudge_core::fuzzy;
use fudge_core::{Action, Config, Icon, Provider, Query, ResultItem};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 500_000;
const MAX_WATCHES: usize = 20_000;
const WATCH_DEPTH: usize = 6;
const FUZZY_BUDGET: Duration = Duration::from_millis(25);

#[derive(Debug, Clone)]
pub struct Entry {
    path: Box<str>,
    name_lower: Box<str>,
    name_start: u32,
    is_dir: bool,
    mtime: u32,
}

impl Entry {
    fn new(path: &Path, is_dir: bool, mtime: u32) -> Option<Entry> {
        let p = path.to_str()?;
        let name = path.file_name()?.to_str()?;
        Some(Entry {
            name_start: (p.len() - name.len()) as u32,
            path: p.into(),
            name_lower: name.to_lowercase().into(),
            is_dir,
            mtime,
        })
    }
    fn name(&self) -> &str {
        &self.path[self.name_start as usize..]
    }
}

#[derive(Clone)]
struct Roots {
    /// (root, max depth)
    roots: Vec<(PathBuf, usize)>,
    exclude: HashSet<String>,
}

impl Roots {
    fn from_config(cfg: &Config) -> Roots {
        let mut roots: Vec<(PathBuf, usize)> = cfg
            .file_search_roots
            .iter()
            .filter(|p| p.is_dir())
            .map(|p| (p.clone(), MAX_DEPTH))
            .collect();
        if cfg.file_search_home_shallow {
            if let Some(h) = dirs::home_dir() {
                roots.push((h, 1));
            }
        }
        Roots { roots, exclude: cfg.file_search_exclude.iter().cloned().collect() }
    }

    fn component_ok(&self, name: &str) -> bool {
        !name.starts_with('.') && !self.exclude.contains(name)
    }

    /// Is `path` inside one of the roots, within depth, with no hidden/excluded component?
    fn allows(&self, path: &Path) -> bool {
        self.roots.iter().any(|(root, depth)| {
            let Ok(rel) = path.strip_prefix(root) else { return false };
            let mut n = 0;
            for c in rel.components() {
                n += 1;
                match c.as_os_str().to_str() {
                    Some(s) if self.component_ok(s) => {}
                    _ => return false,
                }
            }
            n >= 1 && n <= *depth
        })
    }
}

fn mtime_of(md: &std::fs::Metadata) -> u32 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs().min(u32::MAX as u64) as u32)
        .unwrap_or(0)
}

enum Msg {
    Fs(Event),
    Rescan,
}

struct Shared {
    entries: RwLock<Vec<Entry>>,
}

fn scan(roots: &Roots, watcher: &mut Option<RecommendedWatcher>, watched: &mut HashSet<PathBuf>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    'roots: for (root, depth) in &roots.roots {
        let walker = walkdir::WalkDir::new(root)
            .max_depth(*depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                e.depth() == 0 || e.file_name().to_str().is_some_and(|n| roots.component_ok(n))
            });
        for entry in walker.flatten() {
            if entry.depth() == 0 {
                if let Some(w) = watcher.as_mut() {
                    if watched.len() < MAX_WATCHES && watched.insert(root.clone()) {
                        let _ = w.watch(root, RecursiveMode::NonRecursive);
                    }
                }
                continue;
            }
            if out.len() >= MAX_ENTRIES {
                break 'roots;
            }
            let is_dir = entry.file_type().is_dir();
            if is_dir && entry.depth() <= WATCH_DEPTH {
                if let Some(w) = watcher.as_mut() {
                    if watched.len() < MAX_WATCHES && watched.insert(entry.path().to_path_buf()) {
                        let _ = w.watch(entry.path(), RecursiveMode::NonRecursive);
                    }
                }
            }
            if !seen.insert(entry.path().to_path_buf()) {
                continue;
            }
            let mtime = entry.metadata().map(|m| mtime_of(&m)).unwrap_or(0);
            if let Some(e) = Entry::new(entry.path(), is_dir, mtime) {
                out.push(e);
            }
        }
    }
    out
}

fn apply_event(
    ev: Event,
    roots: &Roots,
    shared: &Shared,
    watcher: &mut Option<RecommendedWatcher>,
    watched: &mut HashSet<PathBuf>,
) {
    use notify::event::ModifyKind;
    let removal = matches!(ev.kind, EventKind::Remove(_));
    let upsert = matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)));
    if !removal && !upsert {
        return;
    }
    for path in ev.paths {
        if !roots.allows(&path) {
            continue;
        }
        let md = if removal { None } else { std::fs::symlink_metadata(&path).ok() };
        let mut entries = shared.entries.write().unwrap();
        let p = path.to_str().unwrap_or("");
        match md {
            Some(md) => {
                if entries.iter().any(|e| &*e.path == p) {
                    continue;
                }
                if entries.len() >= MAX_ENTRIES {
                    continue;
                }
                if let Some(e) = Entry::new(&path, md.is_dir(), mtime_of(&md)) {
                    entries.push(e);
                }
                if md.is_dir() {
                    if let Some(w) = watcher.as_mut() {
                        if watched.len() < MAX_WATCHES && watched.insert(path.clone()) {
                            let _ = w.watch(&path, RecursiveMode::NonRecursive);
                        }
                    }
                }
            }
            None => {
                let prefix = format!("{p}/");
                entries.retain(|e| &*e.path != p && !e.path.starts_with(&prefix));
                watched.remove(&path);
            }
        }
    }
}

fn worker(rx: Receiver<Msg>, tx: Sender<Msg>, roots: Roots, shared: Arc<Shared>, epoch: Arc<AtomicU64>) {
    let mut watcher: Option<RecommendedWatcher> = {
        let tx = tx.clone();
        notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(ev) = res {
                let _ = tx.send(Msg::Fs(ev));
            }
        })
        .map_err(|e| log::warn!("file watcher unavailable: {e}"))
        .ok()
    };
    let mut watched = HashSet::new();
    let started = Instant::now();
    let entries = scan(&roots, &mut watcher, &mut watched);
    log::debug!("files: indexed {} entries in {:?}", entries.len(), started.elapsed());
    *shared.entries.write().unwrap() = entries;
    crate::bump(&epoch);

    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Fs(ev) => apply_event(ev, &roots, &shared, &mut watcher, &mut watched),
            Msg::Rescan => {
                let entries = scan(&roots, &mut watcher, &mut watched);
                *shared.entries.write().unwrap() = entries;
                crate::bump(&epoch);
            }
        }
    }
}

pub fn icon_for(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    let mime = mime_guess::from_path(path).first_raw().unwrap_or("");
    match mime {
        "application/pdf" => "application-pdf",
        "application/zip" | "application/x-tar" | "application/gzip" | "application/x-7z-compressed"
        | "application/x-rar-compressed" | "application/x-xz" | "application/x-bzip2" => "package-x-generic",
        "application/vnd.oasis.opendocument.spreadsheet" | "application/vnd.ms-excel"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" | "text/csv" => "x-office-spreadsheet",
        "application/vnd.oasis.opendocument.text" | "application/msword"
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "x-office-document",
        "application/vnd.oasis.opendocument.presentation" | "application/vnd.ms-powerpoint"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "x-office-presentation",
        m if m.starts_with("image/") => "image-x-generic",
        m if m.starts_with("audio/") => "audio-x-generic",
        m if m.starts_with("video/") => "video-x-generic",
        m if m.starts_with("text/") => "text-x-generic",
        _ => "text-x-generic",
    }
}

pub struct FilesProvider {
    shared: Arc<Shared>,
    tx: Option<Sender<Msg>>,
}

impl FilesProvider {
    pub fn new(cfg: &Config, epoch: Arc<AtomicU64>) -> Self {
        let roots = Roots::from_config(cfg);
        let shared = Arc::new(Shared { entries: RwLock::new(Vec::new()) });
        let (tx, rx) = channel();
        let (s2, tx2) = (shared.clone(), tx.clone());
        let spawned = std::thread::Builder::new()
            .name("fudge-files-index".into())
            .spawn(move || worker(rx, tx2, roots, s2, epoch));
        if let Err(e) = spawned {
            log::warn!("cannot start file indexer: {e}");
            return Self { shared, tx: None };
        }
        Self { shared, tx: Some(tx) }
    }

    #[cfg(test)]
    fn with_entries(paths: &[(&str, bool, u32)]) -> Self {
        let entries = paths.iter().filter_map(|(p, d, m)| Entry::new(Path::new(p), *d, *m)).collect();
        Self { shared: Arc::new(Shared { entries: RwLock::new(entries) }), tx: None }
    }

    pub fn len(&self) -> usize {
        self.shared.entries.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn search_index(&self, text: &str, now: u64) -> Vec<(f32, Entry)> {
        let words: Vec<String> = text.to_lowercase().split_whitespace().map(str::to_string).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let entries = self.shared.entries.read().unwrap();

        // Pass 1: every word is a substring of the name (fast).
        let mut hits: Vec<&Entry> = entries
            .iter()
            .filter(|e| words.iter().all(|w| e.name_lower.contains(w.as_str())))
            .take(3000)
            .collect();

        // Pass 2: typo-tolerant fuzzy over names, time-boxed, only when substring hits are scarce.
        if hits.len() < 8 && text.chars().count() >= 3 {
            let start = Instant::now();
            let have: HashSet<*const Entry> = hits.iter().map(|e| *e as *const Entry).collect();
            for (i, e) in entries.iter().enumerate() {
                if i % 2048 == 0 && start.elapsed() > FUZZY_BUDGET {
                    break;
                }
                if have.contains(&(e as *const Entry)) {
                    continue;
                }
                if fuzzy::score(text, e.name()).is_some_and(|s| s >= 55.0) {
                    hits.push(e);
                    if hits.len() >= 64 {
                        break;
                    }
                }
            }
        }

        let mut scored: Vec<(f32, Entry)> = hits
            .into_iter()
            .filter_map(|e| {
                let mut s = fuzzy::score(text, e.name())?;
                let age = now.saturating_sub(e.mtime as u64);
                if e.mtime != 0 {
                    if age < 7 * 86400 {
                        s += 6.0;
                    } else if age < 30 * 86400 {
                        s += 3.0;
                    }
                }
                if e.is_dir {
                    s += 2.0;
                }
                Some((s.min(100.0) * 0.85, e.clone()))
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(12);
        scored
    }
}

fn to_item(score: f32, path: &Path, is_dir: bool) -> ResultItem {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let parent = path.parent().map(crate::tilde).unwrap_or_default();
    ResultItem::new(format!("file:{}", path.display()), name, "files")
        .subtitle(parent)
        .icon(Icon::Named(icon_for(path, is_dir).into()))
        .score(score)
        .action(Action::Open(path.to_path_buf()))
        .action(Action::RevealInFolder(path.to_path_buf()))
}

impl Provider for FilesProvider {
    fn id(&self) -> &'static str {
        "files"
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        if q.keyword.is_some() || q.text.chars().count() < 2 {
            return Vec::new();
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        #[allow(unused_mut)]
        let mut items: Vec<ResultItem> = self
            .search_index(&q.text, now)
            .into_iter()
            .map(|(s, e)| to_item(s, Path::new(&*e.path), e.is_dir))
            .collect();

        #[cfg(target_os = "macos")]
        if q.text.chars().count() >= 3 {
            for path in mdfind(&q.text, Duration::from_millis(50)) {
                let id = format!("file:{}", path.display());
                if items.iter().any(|i| i.id == id) {
                    continue;
                }
                let is_dir = path.is_dir();
                let s = fuzzy::score(&q.text, &path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    .unwrap_or(30.0)
                    * 0.8;
                items.push(to_item(s, &path, is_dir));
            }
        }
        items
    }

    fn refresh(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Rescan);
        }
    }
}

/// Spotlight name search with a hard timeout.
#[cfg(target_os = "macos")]
fn mdfind(text: &str, timeout: Duration) -> Vec<PathBuf> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new("mdfind")
        .args(["-name", text])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
    else {
        return Vec::new();
    };
    let mut stdout = child.stdout.take();
    let (tx, rx) = channel::<String>();
    std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = stdout.as_mut() {
            let _ = o.read_to_string(&mut s);
        }
        let _ = tx.send(s);
    });
    match rx.recv_timeout(timeout) {
        Ok(s) => {
            let _ = child.wait();
            s.lines().take(10).map(PathBuf::from).collect()
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(s: &str) -> Query {
        Query::parse(s, |_| false)
    }

    #[test]
    fn finds_by_name_and_builds_actions() {
        let p = FilesProvider::with_entries(&[
            ("/home/u/Documents/budget-2026.ods", false, 0),
            ("/home/u/Documents/notes.txt", false, 0),
            ("/home/u/Pictures/cat.png", false, 0),
        ]);
        let r = p.query(&q("budget"));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "budget-2026.ods");
        assert_eq!(r[0].id, "file:/home/u/Documents/budget-2026.ods");
        assert!(matches!(r[0].actions[0], Action::Open(_)));
        assert!(matches!(r[0].actions[1], Action::RevealInFolder(_)));
        assert_eq!(r[0].icon, Icon::Named("x-office-spreadsheet".into()));
    }

    #[test]
    fn short_and_keyword_queries_are_empty() {
        let p = FilesProvider::with_entries(&[("/a/bb", false, 0)]);
        assert!(p.query(&q("b")).is_empty());
        assert!(p.query(&Query::parse("kill bb", |k| k == "kill")).is_empty());
    }

    #[test]
    fn prefix_outranks_substring() {
        let p = FilesProvider::with_entries(&[
            ("/x/old-report.txt", false, 0),
            ("/x/report.txt", false, 0),
        ]);
        let r = p.query(&q("report"));
        assert_eq!(r[0].title, "report.txt");
    }

    #[test]
    fn multi_word_and_dirs() {
        let p = FilesProvider::with_entries(&[
            ("/x/Tax Return 2025.pdf", false, 0),
            ("/x/Taxes", true, 0),
        ]);
        let r = p.query(&q("tax 2025"));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].icon, Icon::Named("application-pdf".into()));
        let r = p.query(&q("taxes"));
        assert_eq!(r[0].icon, Icon::Named("folder".into()));
    }

    #[test]
    fn roots_filtering() {
        let roots = Roots {
            roots: vec![(PathBuf::from("/h/Documents"), 8), (PathBuf::from("/h"), 1)],
            exclude: ["node_modules".to_string()].into_iter().collect(),
        };
        assert!(roots.allows(Path::new("/h/Documents/a/b.txt")));
        assert!(roots.allows(Path::new("/h/file.txt")));
        assert!(!roots.allows(Path::new("/h/Documents/.git/config")));
        assert!(!roots.allows(Path::new("/h/Documents/p/node_modules/x/y.js")));
        assert!(!roots.allows(Path::new("/h/Other/deep/file.txt")));
        assert!(!roots.allows(Path::new("/elsewhere/file")));
    }

    #[test]
    fn icons() {
        assert_eq!(icon_for(Path::new("a.mp3"), false), "audio-x-generic");
        assert_eq!(icon_for(Path::new("a.mkv"), false), "video-x-generic");
        assert_eq!(icon_for(Path::new("a.rs"), false), "text-x-generic");
        assert_eq!(icon_for(Path::new("a.zip"), false), "package-x-generic");
    }
}
