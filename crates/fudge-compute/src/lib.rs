//! Fudge Launcher answers engine: math, units, currency, time, emoji and colors.
//!
//! Pure functions only: no I/O, no system clock, no network. The caller
//! supplies `now` and (optionally) exchange rates through [`Context`].

pub use chrono;

mod calc;
mod color;
mod currency;
mod emoji;
mod format;
mod time;
mod units;

#[cfg(feature = "wasm")]
mod wasm;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerKind {
    Math,
    Unit,
    Currency,
    Time,
    Emoji,
    Color,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputeAnswer {
    pub kind: AnswerKind,
    pub title: String,
    pub subtitle: String,
    pub copy_text: String,
    pub score: u32,
    pub needs_network: bool,
    pub color_hex: Option<String>,
}

pub trait RateSource {
    /// Rates against USD, e.g. {"INR": 83.12, "EUR": 0.92}.
    fn usd_rates(&self) -> Option<std::collections::HashMap<String, f64>>;
}

pub struct Context<'a> {
    pub now: chrono::DateTime<chrono::Utc>,
    pub rates: Option<&'a dyn RateSource>,
    pub network_enabled: bool,
}

/// Inputs longer than this (in bytes) are ignored.
const MAX_INPUT_BYTES: usize = 512;

/// 0..n answers, best first. Fast (<1ms typical), never panics on any input.
pub fn evaluate(input: &str, ctx: &Context) -> Vec<ComputeAnswer> {
    let input = input.trim();
    if input.is_empty() || input.len() > MAX_INPUT_BYTES {
        return Vec::new();
    }
    let mut out: Vec<ComputeAnswer> = Vec::new();
    out.extend(color::evaluate(input));
    if let Some(a) = units::evaluate(input) {
        out.push(a);
    } else {
        out.extend(currency::evaluate(input, ctx));
    }
    out.extend(calc::evaluate(input));
    out.extend(time::evaluate(input, ctx.now));
    out.extend(emoji::evaluate(input));
    // Stable sort keeps each provider's own ordering for equal scores.
    out.sort_by(|a, b| b.score.cmp(&a.score));
    out
}

/// True if input looks like a currency conversion (caller can lazily fetch rates).
pub fn wants_rates(input: &str) -> bool {
    let input = input.trim();
    if input.is_empty() || input.len() > MAX_INPUT_BYTES {
        return false;
    }
    currency::parse(input).is_some() && units::evaluate(input).is_none()
}
