//! Emoji lookup by name or shortcode.

use crate::{AnswerKind, ComputeAnswer};

const MAX_RESULTS: usize = 8;

fn extract_query(input: &str) -> Option<String> {
    let lower = input.trim().to_lowercase();
    let q = if let Some(r) = lower
        .strip_prefix("emoji ")
        .or_else(|| lower.strip_prefix("emojis "))
    {
        r.trim().to_string()
    } else if let Some(r) = lower.strip_prefix(':') {
        let r = r.strip_suffix(':').unwrap_or(r);
        if r.chars().count() < 2
            || !r
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '+' | '-'))
        {
            return None;
        }
        r.to_string()
    } else {
        return None;
    };
    let q = q.replace([' ', '-'], "_");
    if q.is_empty() {
        None
    } else {
        Some(q)
    }
}

/// 0 exact, 1 prefix, 2 word prefix, 3 contains.
fn rank_of(candidate: &str, q: &str) -> Option<u8> {
    if candidate == q {
        Some(0)
    } else if candidate.starts_with(q) {
        Some(1)
    } else if candidate.split('_').any(|w| w.starts_with(q)) {
        Some(2)
    } else if candidate.contains(q) {
        Some(3)
    } else {
        None
    }
}

pub fn evaluate(input: &str) -> Vec<ComputeAnswer> {
    let Some(q) = extract_query(input) else {
        return Vec::new();
    };
    let mut hits: Vec<(u8, usize, &'static emojis::Emoji)> = Vec::new();
    for (idx, e) in emojis::iter().enumerate() {
        let name = e.name().to_lowercase();
        if name.contains("skin tone") {
            continue;
        }
        let name_us = name.replace([' ', '-'], "_");
        let mut best = rank_of(&name_us, &q);
        for sc in e.shortcodes() {
            if let Some(r) = rank_of(&sc.to_lowercase(), &q) {
                best = Some(best.map_or(r, |b| b.min(r)));
            }
        }
        if let Some(r) = best {
            hits.push((r, idx, e));
        }
    }
    hits.sort_by_key(|(r, idx, _)| (*r, *idx));
    hits.into_iter()
        .take(MAX_RESULTS)
        .map(|(r, _, e)| ComputeAnswer {
            kind: AnswerKind::Emoji,
            title: e.as_str().to_string(),
            subtitle: format!("{} \u{b7} Enter copies", e.name()),
            copy_text: e.as_str().to_string(),
            score: 75 - 2 * r as u32,
            needs_network: false,
            color_hex: None,
        })
        .collect()
}
