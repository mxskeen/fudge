//! Currency conversion parsing and answers. Rates come from the caller.

use crate::format::{fmt_fixed, fmt_num, parse_number_prefix};
use crate::{AnswerKind, ComputeAnswer, Context};

static CODES: &[&str] = &[
    "USD", "EUR", "GBP", "JPY", "INR", "CNY", "CAD", "AUD", "CHF", "SEK", "NOK", "DKK", "NZD",
    "SGD", "HKD", "KRW", "MXN", "BRL", "ZAR", "RUB", "TRY", "PLN", "CZK", "HUF", "ILS", "AED",
    "SAR", "THB", "IDR", "MYR", "PHP", "VND", "TWD", "EGP", "NGN", "KES", "ARS", "CLP", "COP",
    "PEN", "PKR", "BDT", "UAH", "RON",
];

static NAMES: &[(&str, &str)] = &[
    ("dollar", "USD"),
    ("us dollar", "USD"),
    ("buck", "USD"),
    ("euro", "EUR"),
    ("rupee", "INR"),
    ("indian rupee", "INR"),
    ("pound", "GBP"),
    ("sterling", "GBP"),
    ("yen", "JPY"),
    ("yuan", "CNY"),
    ("renminbi", "CNY"),
    ("franc", "CHF"),
    ("won", "KRW"),
    ("canadian dollar", "CAD"),
    ("australian dollar", "AUD"),
];

fn symbol(c: char) -> Option<&'static str> {
    match c {
        '$' => Some("USD"),
        '\u{20ac}' => Some("EUR"),
        '\u{a3}' => Some("GBP"),
        '\u{a5}' => Some("JPY"),
        '\u{20b9}' => Some("INR"),
        _ => None,
    }
}

fn code_of(s: &str) -> Option<&'static str> {
    let s = s.trim();
    let mut chars = s.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if let Some(code) = symbol(c) {
            return Some(code);
        }
    }
    let lower = s.to_lowercase();
    let lower = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = lower.to_uppercase();
    if let Some(c) = CODES.iter().find(|c| **c == upper) {
        return Some(*c);
    }
    let by_name = |n: &str| NAMES.iter().find(|(name, _)| *name == n).map(|(_, c)| *c);
    by_name(&lower)
        .or_else(|| lower.strip_suffix('s').and_then(by_name))
        .or_else(|| lower.strip_suffix("es").and_then(by_name))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Conversion {
    pub amount: f64,
    pub from: &'static str,
    pub to: &'static str,
}

fn parse_left(left: &str) -> Option<(f64, &'static str)> {
    let left = left.trim();
    let mut sym = None;
    let mut rest = left;
    if let Some(c) = left.chars().next() {
        if let Some(code) = symbol(c) {
            sym = Some(code);
            rest = &left[c.len_utf8()..];
        }
    }
    let (v, tail) = parse_number_prefix(rest.trim_start())?;
    let tail = tail.trim();
    if tail.is_empty() {
        sym.map(|c| (v, c))
    } else {
        code_of(tail).map(|c| (v, c))
    }
}

pub(crate) fn parse(input: &str) -> Option<Conversion> {
    let toks: Vec<&str> = input.split_whitespace().collect();
    if toks.len() < 3 {
        return None;
    }
    for i in 1..toks.len() - 1 {
        if matches!(toks[i].to_lowercase().as_str(), "to" | "in" | "into" | "as") {
            let left = toks[..i].join(" ");
            let right = toks[i + 1..].join(" ");
            if let (Some((amount, from)), Some(to)) = (parse_left(&left), code_of(&right)) {
                return Some(Conversion { amount, from, to });
            }
        }
    }
    None
}

fn answer(title: String, subtitle: String, copy: String, score: u32, net: bool) -> ComputeAnswer {
    ComputeAnswer {
        kind: AnswerKind::Currency,
        title,
        subtitle,
        copy_text: copy,
        score,
        needs_network: net,
        color_hex: None,
    }
}

pub fn evaluate(input: &str, ctx: &Context) -> Vec<ComputeAnswer> {
    let Some(conv) = parse(input) else {
        return Vec::new();
    };
    if !ctx.network_enabled {
        return vec![answer(
            "Currency conversion is turned off".into(),
            "Turn on network features in Settings \u{2192} Privacy".into(),
            String::new(),
            50,
            false,
        )];
    }
    let rates = ctx.rates.and_then(|r| r.usd_rates());
    let Some(rates) = rates else {
        return vec![answer(
            "Needs today's rates".into(),
            "Connect to the internet to convert currency".into(),
            String::new(),
            50,
            true,
        )];
    };
    let rate_of = |c: &str| -> Option<f64> {
        if c == "USD" {
            return Some(1.0);
        }
        rates
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(c))
            .map(|(_, v)| *v)
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    let (Some(rf), Some(rt)) = (rate_of(conv.from), rate_of(conv.to)) else {
        return vec![answer(
            "Needs today's rates".into(),
            format!("No rate available for {} or {}", conv.from, conv.to),
            String::new(),
            50,
            true,
        )];
    };
    let rate = rt / rf;
    let out = conv.amount * rate;
    if !out.is_finite() {
        return Vec::new();
    }
    let s = fmt_fixed(out, 2);
    vec![answer(
        format!("= {} {}", s, conv.to),
        format!(
            "1 {} = {} {} \u{b7} rates from the internet",
            conv.from,
            fmt_num(rate),
            conv.to
        ),
        s,
        90,
        true,
    )]
}
