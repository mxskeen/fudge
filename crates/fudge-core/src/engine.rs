//! Search orchestration: route, fan out with deadlines, rank, dedupe.

use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fudge_platform::Platform;

use crate::config::Config;
use crate::exec::{self, ExecOutcome};
use crate::history::History;
use crate::item::ResultItem;
use crate::provider::Provider;
use crate::query::Query;

/// Deadline when every provider runs (typing a plain query).
pub const BROAD_DEADLINE: Duration = Duration::from_millis(40);
/// Deadline when a keyword routes to specific providers (they may do real work).
pub const KEYWORD_DEADLINE: Duration = Duration::from_millis(250);
/// Provider id that is queried even when another provider owns the keyword.
pub const COMPUTE_PROVIDER_ID: &str = "compute";

pub struct Engine {
    providers: Vec<Arc<dyn Provider>>,
    history: Arc<History>,
    config: Config,
    epoch: Arc<AtomicU64>,
    keywords: HashMap<String, Vec<usize>>,
}

/// Frecency -> score boost. Saturates so history can reorder but not bury an exact match.
pub fn frecency_boost(f: f32) -> f32 {
    (f.max(0.0).ln_1p() * 12.0).min(36.0)
}

impl Engine {
    pub fn new(providers: Vec<Arc<dyn Provider>>, history: Arc<History>, config: Config) -> Self {
        Self::with_epoch(providers, history, config, Arc::new(AtomicU64::new(0)))
    }

    /// Like `new`, but with an epoch counter shared with providers created earlier.
    pub fn with_epoch(
        providers: Vec<Arc<dyn Provider>>,
        history: Arc<History>,
        config: Config,
        epoch: Arc<AtomicU64>,
    ) -> Self {
        history.set_enabled(config.history_enabled);
        let mut keywords: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, p) in providers.iter().enumerate() {
            for k in p.keywords() {
                keywords.entry(k.to_lowercase()).or_default().push(i);
            }
        }
        Self { providers, history, config, epoch, keywords }
    }

    /// Counter providers bump when background data arrives (rates, index ready);
    /// the UI re-runs the current query when it changes.
    pub fn epoch(&self) -> Arc<AtomicU64> {
        self.epoch.clone()
    }

    pub fn epoch_value(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }

    pub fn history(&self) -> &Arc<History> {
        &self.history
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn parse(&self, raw: &str) -> Query {
        Query::parse(raw, |k| self.keywords.contains_key(k))
    }

    /// Ask every provider to rescan its caches (each on its own thread).
    pub fn refresh_all(&self) {
        for p in &self.providers {
            let p = p.clone();
            std::thread::spawn(move || p.refresh());
        }
    }

    pub fn search(&self, raw: &str, limit: usize) -> Vec<ResultItem> {
        let parsed = self.parse(raw);
        if parsed.is_empty() {
            return self.history.recents(limit);
        }

        let owners = parsed.keyword.as_ref().and_then(|k| self.keywords.get(k));
        let (query, selected, deadline) = match owners {
            Some(idx) => {
                let mut sel = idx.clone();
                for (i, p) in self.providers.iter().enumerate() {
                    if p.id() == COMPUTE_PROVIDER_ID && !sel.contains(&i) {
                        sel.push(i);
                    }
                }
                (parsed, sel, KEYWORD_DEADLINE)
            }
            None => {
                // An unowned "/foo" or similar is just text.
                let q = Query { keyword: None, rest: parsed.text.clone(), ..parsed };
                (q, (0..self.providers.len()).collect(), BROAD_DEADLINE)
            }
        };

        let items = self.run_providers(&selected, &query, deadline);
        self.rank(items, limit)
    }

    fn run_providers(&self, selected: &[usize], q: &Query, deadline: Duration) -> Vec<ResultItem> {
        let (tx, rx) = mpsc::channel::<Vec<ResultItem>>();
        let mut spawned = 0;
        for &i in selected {
            let p = self.providers[i].clone();
            let q = q.clone();
            let tx = tx.clone();
            let ok = std::thread::Builder::new()
                .name(format!("fudge-provider-{}", p.id()))
                .spawn(move || {
                    let res = catch_unwind(AssertUnwindSafe(|| p.query(&q))).unwrap_or_else(|_| {
                        log::error!("provider {} panicked", p.id());
                        Vec::new()
                    });
                    let _ = tx.send(res);
                })
                .is_ok();
            if ok {
                spawned += 1;
            }
        }
        drop(tx);

        let start = Instant::now();
        let mut all = Vec::new();
        for _ in 0..spawned {
            let left = deadline.saturating_sub(start.elapsed());
            match rx.recv_timeout(left) {
                Ok(mut items) => all.append(&mut items),
                Err(_) => break, // timeout or all senders gone: drop late results
            }
        }
        all
    }

    /// provider score + frecency boost, stable sort, dedupe by id, truncate.
    pub fn rank(&self, mut items: Vec<ResultItem>, limit: usize) -> Vec<ResultItem> {
        let ids: Vec<&str> = items.iter().filter(|i| i.record_history).map(|i| i.id.as_str()).collect();
        let fmap = self.history.frecency_many(&ids);
        for it in &mut items {
            if let Some(f) = fmap.get(&it.id) {
                it.score += frecency_boost(*f);
            }
        }
        items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        let mut seen = HashSet::new();
        items.retain(|i| seen.insert(i.id.clone()));
        items.truncate(limit);
        items
    }

    /// Run `item.actions[action_index]`, record history, audit-log risky actions.
    /// The caller must have confirmed first if `Action::needs_confirm()`.
    pub fn run(&self, item: &ResultItem, action_index: usize, platform: &dyn Platform) -> Result<ExecOutcome> {
        let action = item.actions.get(action_index).context("no such action")?;
        exec::execute_with(action, item, platform, &self.history, self.config.terminal.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;

    struct Fixed {
        id: &'static str,
        kws: Vec<&'static str>,
        items: Vec<(&'static str, f32)>,
        delay: Duration,
    }

    impl Provider for Fixed {
        fn id(&self) -> &'static str {
            self.id
        }
        fn keywords(&self) -> &[&'static str] {
            &self.kws
        }
        fn query(&self, q: &Query) -> Vec<ResultItem> {
            std::thread::sleep(self.delay);
            self.items
                .iter()
                .map(|(id, s)| {
                    ResultItem::new(*id, format!("{id} {}", q.rest), self.id)
                        .score(*s)
                        .action(Action::Copy(id.to_string()))
                })
                .collect()
        }
    }

    fn fixed(id: &'static str, kws: Vec<&'static str>, items: Vec<(&'static str, f32)>, ms: u64) -> Arc<dyn Provider> {
        Arc::new(Fixed { id, kws, items, delay: Duration::from_millis(ms) })
    }

    fn engine(ps: Vec<Arc<dyn Provider>>, h: Arc<History>) -> Engine {
        Engine::new(ps, h, Config::default())
    }

    #[test]
    fn ranks_dedupes_truncates() {
        let e = engine(
            vec![
                fixed("a", vec![], vec![("x", 50.0), ("y", 80.0)], 0),
                fixed("b", vec![], vec![("x", 90.0), ("z", 10.0)], 0),
            ],
            Arc::new(History::in_memory()),
        );
        let r = e.search("q", 10);
        let ids: Vec<_> = r.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["x", "y", "z"]);
        assert_eq!(r[0].score, 90.0);
        assert_eq!(e.search("q", 2).len(), 2);
    }

    #[test]
    fn frecency_reorders() {
        let h = Arc::new(History::in_memory());
        let used = ResultItem::new("lo", "lo", "a").action(Action::Copy("lo".into()));
        for _ in 0..20 {
            h.record(&used).unwrap();
        }
        let e = engine(vec![fixed("a", vec![], vec![("hi", 60.0), ("lo", 50.0)], 0)], h);
        let r = e.search("q", 10);
        assert_eq!(r[0].id, "lo");
        assert!(r[0].score > 50.0 && r[0].score <= 50.0 + 36.0);
    }

    #[test]
    fn keyword_routes_exclusively_plus_compute() {
        let e = engine(
            vec![
                fixed("apps", vec![], vec![("app", 70.0)], 0),
                fixed("kill", vec!["kill"], vec![("k", 70.0)], 0),
                fixed("compute", vec![], vec![("c", 70.0)], 0),
            ],
            Arc::new(History::in_memory()),
        );
        let r = e.search("kill foo", 10);
        let mut ids: Vec<_> = r.iter().map(|i| i.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["c", "k"]);
        assert_eq!(r.iter().find(|i| i.id == "k").unwrap().title, "k foo");
        // Unowned slash text is just text for everyone.
        assert_eq!(e.search("/etc/hosts", 10).len(), 2 + 1);
    }

    #[test]
    fn late_providers_are_dropped() {
        let e = engine(
            vec![
                fixed("fast", vec![], vec![("f", 10.0)], 0),
                fixed("slow", vec![], vec![("s", 99.0)], 400),
            ],
            Arc::new(History::in_memory()),
        );
        let start = Instant::now();
        let r = e.search("q", 10);
        assert!(start.elapsed() < Duration::from_millis(300));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "f");
    }

    #[test]
    fn empty_query_returns_recents() {
        let h = Arc::new(History::in_memory());
        h.record(&ResultItem::new("r", "Recent", "a").action(Action::Copy("r".into()))).unwrap();
        let e = engine(vec![fixed("a", vec![], vec![("x", 1.0)], 0)], h);
        let r = e.search("  ", 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "r");
    }

    #[test]
    fn epoch_is_shared() {
        let e = engine(vec![], Arc::new(History::in_memory()));
        let ep = e.epoch();
        ep.fetch_add(1, Ordering::Relaxed);
        assert_eq!(e.epoch_value(), 1);
    }
}
