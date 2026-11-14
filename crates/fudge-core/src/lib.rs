//! Core types and logic of Fudge Launcher: no GUI dependencies, unit-testable.

pub mod action;
pub mod config;
pub mod engine;
pub mod exec;
pub mod fuzzy;
pub mod history;
pub mod item;
pub mod paths;
pub mod provider;
pub mod query;

pub use action::Action;
pub use config::{Config, Style};
pub use engine::Engine;
pub use exec::{execute, execute_with, ExecOutcome};
pub use fudge_platform::{
    DesktopKind, Platform, SessionKind, SystemCommand, WindowId, WindowInfo,
};
pub use history::History;
pub use item::{Icon, ResultItem};
pub use provider::Provider;
pub use query::Query;
