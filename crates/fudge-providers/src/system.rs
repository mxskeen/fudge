//! Lock, sleep, restart, shutdown, log out, empty trash.

use fudge_core::{Action, Icon, Provider, Query, ResultItem, SystemCommand};

struct Entry {
    id: &'static str,
    title: &'static str,
    aliases: &'static [&'static str],
    subtitle: &'static str,
    icon: &'static str,
    cmd: SystemCommand,
    destructive: bool,
}

const ENTRIES: &[Entry] = &[
    Entry { id: "lock", title: "Lock", aliases: &["lock", "lock screen"], subtitle: "Locks the screen", icon: "system-lock-screen", cmd: SystemCommand::Lock, destructive: false },
    Entry { id: "sleep", title: "Sleep", aliases: &["sleep", "suspend"], subtitle: "Suspends the computer", icon: "system-suspend", cmd: SystemCommand::Sleep, destructive: false },
    Entry { id: "restart", title: "Restart", aliases: &["restart", "reboot"], subtitle: "Restarts the computer (asks first)", icon: "system-reboot", cmd: SystemCommand::Restart, destructive: true },
    Entry { id: "shutdown", title: "Shut down", aliases: &["shutdown", "shut down", "power off", "poweroff"], subtitle: "Turns the computer off (asks first)", icon: "system-shutdown", cmd: SystemCommand::Shutdown, destructive: true },
    Entry { id: "logout", title: "Log out", aliases: &["logout", "log out", "sign out", "signout"], subtitle: "Ends your session (asks first)", icon: "system-log-out", cmd: SystemCommand::Logout, destructive: true },
    Entry { id: "empty-trash", title: "Empty trash", aliases: &["empty trash", "empty bin"], subtitle: "Permanently deletes trashed files (asks first)", icon: "user-trash", cmd: SystemCommand::EmptyTrash, destructive: true },
];

/// Prefix score of `text` against an alias or any of its words.
fn alias_score(text: &str, alias: &str) -> Option<f32> {
    if alias == text {
        return Some(100.0);
    }
    if alias.starts_with(text) {
        return Some(90.0 - (alias.len() - text.len()) as f32 * 0.5);
    }
    if text.len() >= 3 && alias.split_whitespace().skip(1).any(|w| w.starts_with(text)) {
        return Some(70.0);
    }
    None
}

#[derive(Default)]
pub struct SystemProvider;

impl SystemProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Provider for SystemProvider {
    fn id(&self) -> &'static str {
        "system"
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        let text = q.text.to_lowercase();
        if q.keyword.is_some() || text.chars().count() < 2 {
            return Vec::new();
        }
        ENTRIES
            .iter()
            .filter_map(|e| {
                let s = e.aliases.iter().filter_map(|a| alias_score(&text, a)).fold(None, |m: Option<f32>, s| Some(m.map_or(s, |m| m.max(s))))?;
                let mut item = ResultItem::new(format!("sys:{}", e.id), e.title, "system")
                    .subtitle(e.subtitle)
                    .icon(Icon::Named(e.icon.into()))
                    .score(s)
                    .action(Action::System(e.cmd));
                // Never surface irreversible commands as "recent" suggestions.
                item.record_history = !e.destructive;
                Some(item)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> Vec<ResultItem> {
        SystemProvider::new().query(&Query::parse(s, |_| false))
    }

    #[test]
    fn prefix_matches() {
        let r = run("loc");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Lock");
        assert_eq!(r[0].actions, vec![Action::System(SystemCommand::Lock)]);
        assert!(r[0].record_history);
    }

    #[test]
    fn aliases_and_words() {
        assert_eq!(run("reboot")[0].title, "Restart");
        assert_eq!(run("power")[0].title, "Shut down");
        assert_eq!(run("trash")[0].title, "Empty trash");
        assert_eq!(run("suspend")[0].title, "Sleep");
        let r = run("lo");
        assert!(r.iter().any(|i| i.title == "Lock") && r.iter().any(|i| i.title == "Log out"));
    }

    #[test]
    fn destructive_not_recorded_and_unrelated_empty() {
        assert!(!run("shutdown")[0].record_history);
        assert!(run("firefox").is_empty());
        assert!(run("l").is_empty());
    }
}
