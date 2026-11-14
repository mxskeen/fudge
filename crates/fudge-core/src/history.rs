//! Local usage history (SQLite) used for frecency ranking and Recents.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::action::Action;
use crate::item::{Icon, ResultItem};

const HALF_LIFE_SECS: f64 = 7.0 * 24.0 * 3600.0;
const MAX_ROWS: i64 = 2000;

pub struct History {
    conn: Mutex<Connection>,
    enabled: AtomicBool,
}

pub fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// count x 0.5^(age / half-life)
pub fn decay(count: i64, last_used: i64, now: i64) -> f32 {
    let age = (now - last_used).max(0) as f64;
    (count as f64 * 0.5f64.powf(age / HALF_LIFE_SECS)) as f32
}

impl History {
    pub fn default_path() -> Option<std::path::PathBuf> {
        crate::paths::data_dir().map(|d| d.join("history.sqlite"))
    }

    pub fn open_default() -> Result<Self> {
        let path = Self::default_path().context("no data directory")?;
        Self::open(&path)
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::from_conn(Connection::open(path)?)
    }

    pub fn in_memory() -> Self {
        Self::from_conn(Connection::open_in_memory().expect("in-memory sqlite")).expect("schema")
    }

    fn from_conn(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                subtitle TEXT NOT NULL DEFAULT '',
                icon_json TEXT NOT NULL,
                action_json TEXT NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                last_used INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS history_last_used ON history(last_used);
             CREATE TABLE IF NOT EXISTS audit (
                ts INTEGER NOT NULL,
                kind TEXT NOT NULL,
                detail TEXT NOT NULL
             );",
        )?;
        Ok(Self { conn: Mutex::new(conn), enabled: AtomicBool::new(true) })
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn record(&self, item: &ResultItem) -> Result<()> {
        self.record_at(item, now_secs())
    }

    pub fn record_at(&self, item: &ResultItem, ts: i64) -> Result<()> {
        if !item.record_history || !self.is_enabled() || item.id.is_empty() {
            return Ok(());
        }
        let icon = serde_json::to_string(&item.icon)?;
        let actions = serde_json::to_string(&item.actions)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO history (id, title, subtitle, icon_json, action_json, count, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title, subtitle = excluded.subtitle,
               icon_json = excluded.icon_json, action_json = excluded.action_json,
               count = count + 1, last_used = excluded.last_used",
            params![item.id, item.title, item.subtitle, icon, actions, ts],
        )?;
        // Keep the table bounded: drop the least recently used rows.
        conn.execute(
            "DELETE FROM history WHERE id IN
               (SELECT id FROM history ORDER BY last_used DESC LIMIT -1 OFFSET ?1)",
            params![MAX_ROWS],
        )?;
        Ok(())
    }

    pub fn frecency(&self, id: &str) -> f32 {
        self.frecency_at(id, now_secs())
    }

    pub fn frecency_at(&self, id: &str, now: i64) -> f32 {
        self.frecency_many_at(&[id], now).get(id).copied().unwrap_or(0.0)
    }

    pub fn frecency_many(&self, ids: &[&str]) -> HashMap<String, f32> {
        self.frecency_many_at(ids, now_secs())
    }

    pub fn frecency_many_at(&self, ids: &[&str], now: i64) -> HashMap<String, f32> {
        let mut out = HashMap::new();
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare_cached("SELECT count, last_used FROM history WHERE id = ?1") else {
            return out;
        };
        for id in ids {
            if let Ok((count, last)) = stmt.query_row(params![id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))) {
                out.insert((*id).to_string(), decay(count, last, now));
            }
        }
        out
    }

    /// Most relevant recent items (by frecency), reconstructed from storage.
    pub fn recents(&self, n: usize) -> Vec<ResultItem> {
        self.recents_at(n, now_secs())
    }

    pub fn recents_at(&self, n: usize, now: i64) -> Vec<ResultItem> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, title, subtitle, icon_json, action_json, count, last_used
             FROM history ORDER BY last_used DESC LIMIT 200",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
            ))
        });
        let Ok(rows) = rows else { return Vec::new() };
        let mut items: Vec<(f32, ResultItem)> = Vec::new();
        for row in rows.flatten() {
            let (id, title, subtitle, icon_json, action_json, count, last) = row;
            let icon: Icon = serde_json::from_str(&icon_json).unwrap_or_default();
            let actions: Vec<Action> = serde_json::from_str(&action_json).unwrap_or_default();
            if actions.is_empty() {
                continue;
            }
            let f = decay(count, last, now);
            items.push((
                f,
                ResultItem {
                    id,
                    title,
                    subtitle,
                    icon,
                    score: f,
                    provider: "history".into(),
                    actions,
                    record_history: true,
                },
            ));
        }
        items.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        items.into_iter().take(n).map(|(_, i)| i).collect()
    }

    pub fn clear(&self) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM history", [])?;
        Ok(())
    }

    /// Append to the audit log (RunShell and confirmed destructive actions).
    pub fn audit(&self, kind: &str, detail: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("INSERT INTO audit (ts, kind, detail) VALUES (?1, ?2, ?3)", params![now_secs(), kind, detail])?;
        Ok(())
    }

    /// Newest-first audit entries as (timestamp, kind, detail).
    pub fn audit_entries(&self, n: usize) -> Vec<(i64, String, String)> {
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT ts, kind, detail FROM audit ORDER BY rowid DESC LIMIT ?1") else {
            return Vec::new();
        };
        stmt.query_map(params![n as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> ResultItem {
        ResultItem::new(id, format!("Title {id}"), "test").action(Action::Copy(id.into()))
    }

    #[test]
    fn record_and_frecency() {
        let h = History::in_memory();
        let now = 1_000_000;
        assert_eq!(h.frecency_at("a", now), 0.0);
        h.record_at(&item("a"), now).unwrap();
        h.record_at(&item("a"), now).unwrap();
        assert!((h.frecency_at("a", now) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn decays_with_half_life() {
        let h = History::in_memory();
        h.record_at(&item("a"), 0).unwrap();
        let week = 7 * 24 * 3600;
        assert!((h.frecency_at("a", week) - 0.5).abs() < 1e-3);
        assert!((h.frecency_at("a", 2 * week) - 0.25).abs() < 1e-3);
    }

    #[test]
    fn respects_flags() {
        let h = History::in_memory();
        let mut i = item("a");
        i.record_history = false;
        h.record(&i).unwrap();
        assert!(h.recents(10).is_empty());
        h.set_enabled(false);
        h.record(&item("b")).unwrap();
        assert!(h.recents(10).is_empty());
        h.set_enabled(true);
        h.record(&item("b")).unwrap();
        assert_eq!(h.recents(10).len(), 1);
    }

    #[test]
    fn recents_ordered_and_clear() {
        let h = History::in_memory();
        let now = now_secs();
        h.record_at(&item("old"), now - 30 * 24 * 3600).unwrap();
        h.record_at(&item("new"), now).unwrap();
        let r = h.recents(5);
        assert_eq!(r[0].id, "new");
        assert_eq!(r[1].id, "old");
        assert_eq!(r[0].actions, vec![Action::Copy("new".into())]);
        assert_eq!(h.recents(1).len(), 1);
        h.clear().unwrap();
        assert!(h.recents(5).is_empty());
    }

    #[test]
    fn audit_log() {
        let h = History::in_memory();
        h.audit("shell", "ls").unwrap();
        h.audit("kill", "spotify").unwrap();
        let e = h.audit_entries(10);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].1, "kill");
    }
}
