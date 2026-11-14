//! Fuzzy scoring on top of `nucleo-matcher`, tuned for launcher use.
//!
//! Scores are 0..=100. Beyond nucleo's own score we boost exact matches,
//! prefixes, word prefixes and word-initial acronyms ("vsc" -> "Visual Studio Code").

use std::cell::RefCell;

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

thread_local! {
    static MATCHER: RefCell<Matcher> = RefCell::new(Matcher::new(Config::DEFAULT));
}

fn words(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect()
}

/// Score `pattern` against `haystack`; `None` if it does not match at all.
pub fn score(pattern: &str, haystack: &str) -> Option<f32> {
    let pattern = pattern.trim();
    if pattern.is_empty() || haystack.is_empty() {
        return None;
    }
    let raw = MATCHER.with(|m| {
        let mut m = m.borrow_mut();
        let pat = Pattern::new(pattern, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
        let mut buf = Vec::new();
        pat.score(Utf32Str::new(haystack, &mut buf), &mut m)
    })?;

    let n = pattern.chars().filter(|c| !c.is_whitespace()).count().max(1) as f32;
    let mut s = (raw as f32 / (n * 28.0)).min(1.0) * 50.0;

    let p = pattern.to_lowercase();
    let h = haystack.to_lowercase();
    if h == p {
        s += 50.0;
    } else if h.starts_with(&p) {
        s += 32.0;
    } else {
        let ws = words(&h);
        if ws.iter().any(|w| w.starts_with(&p)) {
            s += 20.0;
        } else if !p.contains(char::is_whitespace) {
            let initials: String = ws.iter().filter_map(|w| w.chars().next()).collect();
            if initials.len() >= 2 && initials.starts_with(&p) {
                s += 28.0;
            } else if h.contains(&p) {
                s += 10.0;
            }
        } else if h.contains(&p) {
            s += 10.0;
        }
    }
    Some(s.min(100.0))
}

/// Best score of `pattern` over several haystacks.
pub fn best<'a>(pattern: &str, hays: impl IntoIterator<Item = &'a str>) -> Option<f32> {
    hays.into_iter()
        .filter_map(|h| score(pattern, h))
        .fold(None, |acc, s| Some(acc.map_or(s, |a: f32| a.max(s))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vsc_finds_visual_studio_code() {
        let a = score("vsc", "Visual Studio Code").unwrap();
        let b = score("vsc", "Evince Scanner Console").unwrap_or(0.0);
        assert!(a > b, "{a} <= {b}");
        assert!(a > 60.0);
    }

    #[test]
    fn prefix_beats_scattered() {
        let a = score("fire", "Firefox").unwrap();
        let b = score("fire", "Fantastic Iron Red Editor").unwrap_or(0.0);
        assert!(a > b);
    }

    #[test]
    fn exact_beats_prefix() {
        assert!(score("code", "Code").unwrap() > score("code", "Code Editor").unwrap());
    }

    #[test]
    fn case_insensitive_and_no_match() {
        assert!(score("FIREFOX", "firefox").is_some());
        assert!(score("zzz", "Firefox").is_none());
        assert!(score("", "x").is_none());
    }

    #[test]
    fn best_of_many() {
        let s = best("term", ["Files", "Terminal"]).unwrap();
        assert!(s > 60.0);
    }
}
