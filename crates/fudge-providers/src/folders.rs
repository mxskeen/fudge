//! Quick folders: home, downloads, documents, desktop, pictures, music, videos, trash.

use std::path::PathBuf;

use fudge_core::fuzzy;
use fudge_core::{Action, Icon, Provider, Query, ResultItem};

struct Folder {
    name: &'static str,
    icon: &'static str,
    path: PathBuf,
}

fn folders() -> Vec<Folder> {
    let trash = if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join(".Trash"))
    } else {
        dirs::data_dir().map(|d| d.join("Trash/files"))
    };
    let list: [(&'static str, &'static str, Option<PathBuf>); 8] = [
        ("Home", "user-home", dirs::home_dir()),
        ("Downloads", "folder-download", dirs::download_dir()),
        ("Documents", "folder-documents", dirs::document_dir()),
        ("Desktop", "user-desktop", dirs::desktop_dir()),
        ("Pictures", "folder-pictures", dirs::picture_dir()),
        ("Music", "folder-music", dirs::audio_dir()),
        ("Videos", "folder-videos", dirs::video_dir()),
        ("Trash", "user-trash", trash),
    ];
    list.into_iter()
        .filter_map(|(name, icon, p)| p.map(|path| Folder { name, icon, path }))
        .collect()
}

pub struct QuickFoldersProvider {
    folders: Vec<Folder>,
    keywords: Vec<&'static str>,
}

impl QuickFoldersProvider {
    pub fn new() -> Self {
        Self { folders: folders(), keywords: vec!["folders"] }
    }

    fn item(f: &Folder, score: f32) -> ResultItem {
        // Same id scheme as the file index so a folder never shows up twice.
        ResultItem::new(format!("file:{}", f.path.display()), f.name, "folders")
            .subtitle(crate::tilde(&f.path))
            .icon(Icon::Named(f.icon.into()))
            .score(score)
            .action(Action::Open(f.path.clone()))
            .action(Action::RevealInFolder(f.path.clone()))
    }
}

impl Default for QuickFoldersProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for QuickFoldersProvider {
    fn id(&self) -> &'static str {
        "folders"
    }

    fn keywords(&self) -> &[&'static str] {
        &self.keywords
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        match q.keyword.as_deref() {
            Some("folders") => self
                .folders
                .iter()
                .filter_map(|f| {
                    if q.rest.is_empty() {
                        Some(Self::item(f, 70.0))
                    } else {
                        fuzzy::score(&q.rest, f.name).map(|s| Self::item(f, s))
                    }
                })
                .collect(),
            Some(_) => Vec::new(),
            None => {
                if q.text.chars().count() < 2 {
                    return Vec::new();
                }
                self.folders
                    .iter()
                    .filter_map(|f| {
                        let s = fuzzy::score(&q.text, f.name)?;
                        (s >= 60.0).then(|| Self::item(f, s * 0.9))
                    })
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_all_and_filters() {
        let p = QuickFoldersProvider::new();
        let kw = |s: &str| s == "folders";
        let all = p.query(&Query::parse("folders", kw));
        assert_eq!(all.len(), p.folders.len());
        if p.folders.iter().any(|f| f.name == "Downloads") {
            let r = p.query(&Query::parse("down", kw));
            assert!(r.iter().any(|i| i.title == "Downloads"));
            assert!(matches!(r[0].actions[0], Action::Open(_)));
        }
        assert!(p.query(&Query::parse("d", kw)).is_empty());
    }
}
