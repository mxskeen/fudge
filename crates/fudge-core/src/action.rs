use std::path::PathBuf;

use fudge_platform::{SystemCommand, WindowId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Open(PathBuf),
    RevealInFolder(PathBuf),
    /// Desktop-file id on Linux, bundle path on macOS.
    LaunchApp { id: String },
    OpenUrl(String),
    Copy(String),
    RunShell { command: String, in_terminal: bool },
    KillProcesses { pids: Vec<u32>, name: String, protected: bool },
    System(SystemCommand),
    FocusWindow(WindowId),
    OpenSettingsPanel(String),
    Plugin { plugin_id: String, payload: serde_json::Value },
}

impl Action {
    /// Whether the UI must ask before running this.
    pub fn needs_confirm(&self) -> bool {
        match self {
            Action::KillProcesses { protected, .. } => *protected,
            Action::System(c) => matches!(
                c,
                SystemCommand::Restart
                    | SystemCommand::Shutdown
                    | SystemCommand::Logout
                    | SystemCommand::EmptyTrash
            ),
            _ => false,
        }
    }

    /// Short verb for the hint bar ("Open", "Copy", "Kill", ...).
    pub fn label(&self) -> &'static str {
        match self {
            Action::Open(_) => "Open",
            Action::RevealInFolder(_) => "Show in folder",
            Action::LaunchApp { .. } => "Launch",
            Action::OpenUrl(_) => "Open",
            Action::Copy(_) => "Copy",
            Action::RunShell { in_terminal: true, .. } => "Run in terminal",
            Action::RunShell { .. } => "Run",
            Action::KillProcesses { .. } => "Kill",
            Action::System(c) => match c {
                SystemCommand::Lock => "Lock",
                SystemCommand::Sleep => "Sleep",
                SystemCommand::Restart => "Restart",
                SystemCommand::Shutdown => "Shut down",
                SystemCommand::Logout => "Log out",
                SystemCommand::EmptyTrash => "Empty trash",
            },
            Action::FocusWindow(_) => "Switch",
            Action::OpenSettingsPanel(_) => "Open",
            Action::Plugin { .. } => "Run",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_rules() {
        assert!(Action::System(SystemCommand::Shutdown).needs_confirm());
        assert!(Action::System(SystemCommand::EmptyTrash).needs_confirm());
        assert!(!Action::System(SystemCommand::Lock).needs_confirm());
        assert!(!Action::System(SystemCommand::Sleep).needs_confirm());
        let k = |protected| Action::KillProcesses { pids: vec![1], name: "x".into(), protected };
        assert!(k(true).needs_confirm());
        assert!(!k(false).needs_confirm());
        assert!(!Action::Copy("a".into()).needs_confirm());
    }

    #[test]
    fn labels_and_serde() {
        assert_eq!(Action::Copy("x".into()).label(), "Copy");
        assert_eq!(Action::System(SystemCommand::Logout).label(), "Log out");
        let a = Action::RunShell { command: "ls".into(), in_terminal: true };
        let j = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<Action>(&j).unwrap(), a);
    }
}
