//! `> command`: run in a terminal (Enter) or in the background (Ctrl+Enter).

use fudge_core::{Action, Icon, Provider, Query, ResultItem};

pub struct ShellProvider {
    keywords: Vec<&'static str>,
}

impl ShellProvider {
    pub fn new() -> Self {
        Self { keywords: vec![">"] }
    }
}

impl Default for ShellProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for ShellProvider {
    fn id(&self) -> &'static str {
        "shell"
    }

    fn keywords(&self) -> &[&'static str] {
        &self.keywords
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        if q.keyword.as_deref() != Some(">") || q.rest.is_empty() {
            return Vec::new();
        }
        let cmd = q.rest.clone();
        vec![ResultItem::new(format!("shell:{cmd}"), format!("> {cmd}"), "shell")
            .subtitle("Run in terminal · Ctrl+Enter runs in background")
            .icon(Icon::Named("utilities-terminal".into()))
            .score(100.0)
            .action(Action::RunShell { command: cmd.clone(), in_terminal: true })
            .action(Action::RunShell { command: cmd, in_terminal: false })]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_two_actions() {
        let p = ShellProvider::new();
        let r = p.query(&Query::parse("> htop", |k| k == ">"));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].actions[0], Action::RunShell { command: "htop".into(), in_terminal: true });
        assert_eq!(r[0].actions[1], Action::RunShell { command: "htop".into(), in_terminal: false });
        assert_eq!(r[0].subtitle, "Run in terminal · Ctrl+Enter runs in background");
        assert!(p.query(&Query::parse(">", |k| k == ">")).is_empty());
        assert!(p.query(&Query::parse("htop", |k| k == ">")).is_empty());
    }
}
