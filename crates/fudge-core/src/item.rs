use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::action::Action;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Icon {
    /// Freedesktop icon-theme name.
    Named(String),
    Path(PathBuf),
    Emoji(String),
    /// CSS-style colour such as "#3b82f6".
    Color(String),
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultItem {
    /// Stable key for history, e.g. "app:firefox.desktop".
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: Icon,
    pub score: f32,
    pub provider: String,
    /// `[0]` runs on Enter, `[1]` (if present) on Ctrl+Enter. May be empty for
    /// informational rows; the UI must tolerate that.
    pub actions: Vec<Action>,
    pub record_history: bool,
}

impl ResultItem {
    pub fn new(id: impl Into<String>, title: impl Into<String>, provider: &str) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: String::new(),
            icon: Icon::None,
            score: 0.0,
            provider: provider.to_string(),
            actions: Vec::new(),
            record_history: true,
        }
    }
    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = s.into();
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = i;
        self
    }
    pub fn score(mut self, s: f32) -> Self {
        self.score = s;
        self
    }
    pub fn action(mut self, a: Action) -> Self {
        self.actions.push(a);
        self
    }
    pub fn no_history(mut self) -> Self {
        self.record_history = false;
        self
    }
}
