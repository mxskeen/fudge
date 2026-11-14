/// A parsed user query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    pub raw: String,
    /// Trimmed text.
    pub text: String,
    /// First word (lower-cased) when it is a registered keyword or starts with `/` or `>`.
    pub keyword: Option<String>,
    /// Text after the keyword, trimmed. Equals `text` when there is no keyword.
    pub rest: String,
}

impl Query {
    pub fn parse(raw: &str, is_keyword: impl Fn(&str) -> bool) -> Query {
        let text = raw.trim().to_string();
        let first_end = text.find(char::is_whitespace).unwrap_or(text.len());
        let first = &text[..first_end];
        let lower = first.to_lowercase();

        // ">htop" and "> htop" both mean keyword ">".
        if let Some(stripped) = text.strip_prefix('>') {
            return Query {
                raw: raw.to_string(),
                keyword: Some(">".into()),
                rest: stripped.trim().to_string(),
                text,
            };
        }
        if !first.is_empty() && (first.starts_with('/') || is_keyword(&lower)) {
            return Query {
                raw: raw.to_string(),
                keyword: Some(lower),
                rest: text[first_end..].trim().to_string(),
                text,
            };
        }
        Query { raw: raw.to_string(), rest: text.clone(), text, keyword: None }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kw(s: &str) -> bool {
        matches!(s, "kill" | "win" | "set")
    }

    #[test]
    fn plain() {
        let q = Query::parse("  firefox  ", kw);
        assert_eq!(q.text, "firefox");
        assert_eq!(q.keyword, None);
        assert_eq!(q.rest, "firefox");
    }

    #[test]
    fn registered_keyword() {
        let q = Query::parse("Kill  spotify now", kw);
        assert_eq!(q.keyword.as_deref(), Some("kill"));
        assert_eq!(q.rest, "spotify now");
        let q = Query::parse("kill", kw);
        assert_eq!(q.keyword.as_deref(), Some("kill"));
        assert_eq!(q.rest, "");
        assert_eq!(Query::parse("killer", kw).keyword, None);
    }

    #[test]
    fn slash_and_shell() {
        let q = Query::parse("/yt cats and dogs", kw);
        assert_eq!(q.keyword.as_deref(), Some("/yt"));
        assert_eq!(q.rest, "cats and dogs");
        let q = Query::parse("> htop -d 5", kw);
        assert_eq!(q.keyword.as_deref(), Some(">"));
        assert_eq!(q.rest, "htop -d 5");
        let q = Query::parse(">ls", kw);
        assert_eq!(q.keyword.as_deref(), Some(">"));
        assert_eq!(q.rest, "ls");
    }

    #[test]
    fn empty() {
        assert!(Query::parse("   ", kw).is_empty());
    }
}
