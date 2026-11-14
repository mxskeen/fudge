//! wasm-bindgen entry point for the browser demo.

use crate::{evaluate, Context, RateSource};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

struct MapRates(HashMap<String, f64>);

impl RateSource for MapRates {
    fn usd_rates(&self) -> Option<HashMap<String, f64>> {
        Some(self.0.clone())
    }
}

/// Evaluate `input` and return a JSON array of answers.
/// `rates_json` is a JSON object of rates against USD (or empty / "null").
#[wasm_bindgen]
pub fn evaluate_json(input: &str, now_unix_ms: f64, rates_json: &str) -> String {
    let now = if now_unix_ms.is_finite() {
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_unix_ms as i64)
    } else {
        None
    }
    .unwrap_or_default();
    let rates = serde_json::from_str::<HashMap<String, f64>>(rates_json)
        .ok()
        .map(MapRates);
    let ctx = Context {
        now,
        rates: rates.as_ref().map(|r| r as &dyn RateSource),
        network_enabled: true,
    };
    serde_json::to_string(&evaluate(input, &ctx)).unwrap_or_else(|_| "[]".to_string())
}
