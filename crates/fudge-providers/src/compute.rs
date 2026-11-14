//! Math, units, currency, time, emoji and colour answers from `fudge-compute`.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fudge_compute::{AnswerKind, ComputeAnswer, Context, RateSource};
use fudge_core::{Action, Icon, Provider, Query, ResultItem};

const RATES_URL: &str = "https://open.er-api.com/v6/latest/USD";
const REFRESH_AFTER: Duration = Duration::from_secs(3600);
const RETRY_AFTER: Duration = Duration::from_secs(120);

#[derive(Default)]
struct RatesState {
    rates: Option<HashMap<String, f64>>,
    fetched: Option<Instant>,
    attempted: Option<Instant>,
    in_flight: bool,
}

/// In-memory USD rates, fetched in the background at most once an hour.
pub struct RatesCache {
    state: Arc<Mutex<RatesState>>,
    epoch: Arc<AtomicU64>,
}

impl RatesCache {
    pub fn new(epoch: Arc<AtomicU64>) -> Self {
        Self { state: Arc::new(Mutex::new(RatesState::default())), epoch }
    }

    /// Start a background fetch if rates are missing or stale. Never blocks.
    pub fn ensure_fresh(&self) {
        {
            let mut s = self.state.lock().unwrap();
            if s.in_flight {
                return;
            }
            let fresh = s.fetched.is_some_and(|t| t.elapsed() < REFRESH_AFTER);
            let recently_tried = s.attempted.is_some_and(|t| t.elapsed() < RETRY_AFTER);
            if fresh || recently_tried {
                return;
            }
            s.in_flight = true;
            s.attempted = Some(Instant::now());
        }
        let state = self.state.clone();
        let epoch = self.epoch.clone();
        let spawned = std::thread::Builder::new().name("fudge-rates".into()).spawn(move || {
            let result = fetch_rates();
            let mut s = state.lock().unwrap();
            s.in_flight = false;
            match result {
                Ok(r) => {
                    s.rates = Some(r);
                    s.fetched = Some(Instant::now());
                    drop(s);
                    crate::bump(&epoch);
                }
                Err(e) => log::warn!("fetching currency rates failed: {e:#}"),
            }
        });
        if spawned.is_err() {
            self.state.lock().unwrap().in_flight = false;
        }
    }

    pub fn is_ready(&self) -> bool {
        self.state.lock().unwrap().rates.is_some()
    }

    /// Test hook / offline seeding.
    pub fn set_rates(&self, rates: HashMap<String, f64>) {
        let mut s = self.state.lock().unwrap();
        s.rates = Some(rates);
        s.fetched = Some(Instant::now());
    }
}

impl RateSource for RatesCache {
    fn usd_rates(&self) -> Option<HashMap<String, f64>> {
        self.state.lock().unwrap().rates.clone()
    }
}

fn fetch_rates() -> anyhow::Result<HashMap<String, f64>> {
    let v: serde_json::Value = ureq::get(RATES_URL)
        .timeout(Duration::from_secs(8))
        .call()?
        .into_json()?;
    parse_rates(&v)
}

pub(crate) fn parse_rates(v: &serde_json::Value) -> anyhow::Result<HashMap<String, f64>> {
    if v.get("result").and_then(|r| r.as_str()).is_some_and(|r| r != "success") {
        anyhow::bail!("rates API reported failure");
    }
    let obj = v
        .get("rates")
        .and_then(|r| r.as_object())
        .ok_or_else(|| anyhow::anyhow!("rates API response has no `rates`"))?;
    let mut m: HashMap<String, f64> = obj
        .iter()
        .filter_map(|(k, v)| v.as_f64().map(|f| (k.to_uppercase(), f)))
        .collect();
    m.insert("USD".into(), 1.0);
    Ok(m)
}

pub struct ComputeProvider {
    rates: Arc<RatesCache>,
    network: bool,
}

impl ComputeProvider {
    pub fn new(network_features: bool, epoch: Arc<AtomicU64>) -> Self {
        Self { rates: Arc::new(RatesCache::new(epoch)), network: network_features }
    }

    pub fn rates(&self) -> &Arc<RatesCache> {
        &self.rates
    }
}

fn to_item(a: ComputeAnswer, pending_rates: bool) -> ResultItem {
    let icon = match a.kind {
        AnswerKind::Emoji => Icon::Emoji(a.copy_text.clone()),
        AnswerKind::Color => Icon::Color(a.color_hex.clone().unwrap_or_else(|| a.copy_text.clone())),
        _ => Icon::Named("accessories-calculator".into()),
    };
    let subtitle = if pending_rates { "Fetching today's rates…".to_string() } else { a.subtitle.clone() };
    let tag = match a.kind {
        AnswerKind::Math => "math",
        AnswerKind::Unit => "unit",
        AnswerKind::Currency => "currency",
        AnswerKind::Time => "time",
        AnswerKind::Emoji => "emoji",
        AnswerKind::Color => "color",
    };
    let mut item = ResultItem::new(format!("compute:{tag}:{}", a.copy_text), a.title, "compute")
        .subtitle(subtitle)
        .icon(icon)
        .score(60.0 + a.score.min(100) as f32 * 0.6)
        .action(Action::Copy(a.copy_text));
    item.record_history = !matches!(a.kind, AnswerKind::Currency | AnswerKind::Time);
    item
}

impl Provider for ComputeProvider {
    fn id(&self) -> &'static str {
        "compute"
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        if q.text.is_empty() {
            return Vec::new();
        }
        if self.network && fudge_compute::wants_rates(&q.text) {
            self.rates.ensure_fresh();
        }
        let ctx = Context {
            now: chrono::Utc::now(),
            rates: Some(&*self.rates as &dyn RateSource),
            network_enabled: self.network,
        };
        let ready = self.rates.is_ready();
        fudge_compute::evaluate(&q.text, &ctx)
            .into_iter()
            .map(|a| {
                let pending = a.needs_network && !ready && self.network;
                to_item(a, pending)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rates_response() {
        let v = serde_json::json!({"result":"success","rates":{"USD":1,"inr":83.5,"EUR":0.92}});
        let m = parse_rates(&v).unwrap();
        assert_eq!(m["INR"], 83.5);
        assert_eq!(m["USD"], 1.0);
        assert!(parse_rates(&serde_json::json!({"result":"error"})).is_err());
    }

    #[test]
    fn maps_answers() {
        let a = ComputeAnswer {
            kind: AnswerKind::Currency,
            title: "1,000 INR".into(),
            subtitle: "12 USD".into(),
            copy_text: "1000".into(),
            score: 90,
            needs_network: true,
            color_hex: None,
        };
        let i = to_item(a, true);
        assert_eq!(i.subtitle, "Fetching today's rates…");
        assert!(!i.record_history);
        assert_eq!(i.actions, vec![Action::Copy("1000".into())]);
        assert!(i.score > 100.0);
    }

    #[test]
    fn math_works_offline() {
        let p = ComputeProvider::new(false, Arc::new(AtomicU64::new(0)));
        let r = p.query(&Query::parse("12 * 18", |_| false));
        assert!(r.iter().any(|i| i.actions == vec![Action::Copy("216".into())]), "{r:?}");
    }
}
