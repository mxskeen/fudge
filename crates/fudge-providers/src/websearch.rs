//! `/yt cats` style web searches. Only ever runs on an explicit keyword.

use fudge_core::{Action, Config, Icon, Provider, Query, ResultItem};

pub struct WebSearchProvider {
    /// (keyword, template)
    searches: Vec<(String, String)>,
    keywords: Vec<&'static str>,
}

/// Fill a `{q}` template with the URL-encoded query (appended if no placeholder).
pub fn build_search_url(template: &str, query: &str) -> String {
    let enc = urlencoding::encode(query);
    if template.contains("{q}") {
        template.replace("{q}", &enc)
    } else {
        format!("{template}{enc}")
    }
}

/// Scheme and host of a template, for the "keyword alone" hint.
fn origin(template: &str) -> String {
    let (scheme, rest) = template.split_once("://").unwrap_or(("https", template));
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    format!("{scheme}://{host}")
}

fn engine_name(keyword: &str, template: &str) -> String {
    match keyword {
        "/online" => "DuckDuckGo".into(),
        "/g" => "Google".into(),
        "/yt" => "YouTube".into(),
        "/gh" => "GitHub".into(),
        "/w" => "Wikipedia".into(),
        "/maps" => "Maps".into(),
        "/aur" => "the AUR".into(),
        "/brew" => "Homebrew".into(),
        "/crates" => "crates.io".into(),
        _ => origin(template).split_once("://").map(|(_, h)| h.trim_start_matches("www.").to_string()).unwrap_or_default(),
    }
}

impl WebSearchProvider {
    pub fn new(cfg: &Config) -> Self {
        let searches: Vec<(String, String)> = cfg
            .web_searches
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect();
        // Trait wants `&'static str`; these live for the whole process anyway.
        let keywords = searches.iter().map(|(k, _)| &*Box::leak(k.clone().into_boxed_str())).collect();
        Self { searches, keywords }
    }
}

impl Provider for WebSearchProvider {
    fn id(&self) -> &'static str {
        "websearch"
    }

    fn keywords(&self) -> &[&'static str] {
        &self.keywords
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        let Some(kw) = q.keyword.as_deref() else { return Vec::new() };
        let Some((_, template)) = self.searches.iter().find(|(k, _)| k == kw) else { return Vec::new() };
        let name = engine_name(kw, template);
        if q.rest.is_empty() {
            return vec![ResultItem::new(format!("web:{kw}"), format!("Search {name} for …"), "websearch")
                .subtitle(format!("Type your search after {kw}"))
                .icon(Icon::Named("system-search".into()))
                .score(50.0)
                .action(Action::OpenUrl(origin(template)))
                .no_history()];
        }
        vec![ResultItem::new(format!("web:{kw}:{}", q.rest), format!("Search {name} for “{}”", q.rest), "websearch")
            .subtitle("Opens in your browser")
            .icon(Icon::Named("system-search".into()))
            .score(90.0)
            .action(Action::OpenUrl(build_search_url(template, &q.rest)))
            .no_history()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encoding() {
        assert_eq!(
            build_search_url("https://duckduckgo.com/?q={q}", "rust lifetimes & traits"),
            "https://duckduckgo.com/?q=rust%20lifetimes%20%26%20traits"
        );
        assert_eq!(build_search_url("https://x.test/s/", "a b"), "https://x.test/s/a%20b");
        assert_eq!(build_search_url("https://x.test/?q={q}", "héllo"), "https://x.test/?q=h%C3%A9llo");
    }

    #[test]
    fn keyword_search_and_hint() {
        let p = WebSearchProvider::new(&Config::default());
        assert!(p.keywords().contains(&"/yt"));
        let kws: Vec<String> = p.keywords().iter().map(|s| s.to_string()).collect();
        let kw = |s: &str| kws.iter().any(|k| k == s);
        let r = p.query(&Query::parse("/yt cats", kw));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Search YouTube for “cats”");
        assert_eq!(
            r[0].actions,
            vec![Action::OpenUrl("https://www.youtube.com/results?search_query=cats".into())]
        );
        assert!(!r[0].record_history);
        let hint = p.query(&Query::parse("/yt", kw));
        assert_eq!(hint[0].title, "Search YouTube for …");
        assert_eq!(hint[0].actions, vec![Action::OpenUrl("https://www.youtube.com".into())]);
    }

    #[test]
    fn never_without_keyword() {
        let p = WebSearchProvider::new(&Config::default());
        assert!(p.query(&Query::parse("cats", |_| false)).is_empty());
        assert!(p.query(&Query::parse("/unknown cats", |_| false)).is_empty());
    }
}
