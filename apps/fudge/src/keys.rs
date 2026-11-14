//! Pure key -> UI action mapping, kept free of widgets so it is unit-testable.

use fudge_core::{Action, ResultItem};
use gtk4::gdk::{Key, ModifierType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Up,
    Down,
    /// Run action index (0 = Enter, 1 = Ctrl+Enter) on the selected row.
    RunSelected(usize),
    /// Run first action of the nth visible result (0-based; Alt+1..8).
    RunNth(usize),
    Escape,
    Tab,
}

pub fn map_key(key: Key, mods: ModifierType) -> Option<KeyAction> {
    let ctrl = mods.contains(ModifierType::CONTROL_MASK);
    let alt = mods.contains(ModifierType::ALT_MASK);
    let shift = mods.contains(ModifierType::SHIFT_MASK);
    if alt && !ctrl {
        let n = match key {
            Key::_1 | Key::KP_1 => 0,
            Key::_2 | Key::KP_2 => 1,
            Key::_3 | Key::KP_3 => 2,
            Key::_4 | Key::KP_4 => 3,
            Key::_5 | Key::KP_5 => 4,
            Key::_6 | Key::KP_6 => 5,
            Key::_7 | Key::KP_7 => 6,
            Key::_8 | Key::KP_8 => 7,
            _ => return None,
        };
        return Some(KeyAction::RunNth(n));
    }
    if ctrl && !alt {
        match key {
            Key::j | Key::J | Key::n | Key::N => return Some(KeyAction::Down),
            Key::k | Key::K | Key::p | Key::P => return Some(KeyAction::Up),
            Key::Return | Key::KP_Enter | Key::ISO_Enter => return Some(KeyAction::RunSelected(1)),
            _ => return None,
        }
    }
    if alt {
        return None;
    }
    match key {
        Key::Up | Key::KP_Up => Some(KeyAction::Up),
        Key::Down | Key::KP_Down => Some(KeyAction::Down),
        Key::Return | Key::KP_Enter | Key::ISO_Enter => Some(KeyAction::RunSelected(0)),
        Key::Escape => Some(KeyAction::Escape),
        Key::Tab if !shift => Some(KeyAction::Tab),
        _ => None,
    }
}


/// Text Tab puts in the entry: app names and folder paths only; nothing for answers, kills, etc.
pub fn tab_fill(item: &ResultItem) -> Option<String> {
    match item.actions.first()? {
        Action::LaunchApp { .. } => Some(item.title.clone()),
        Action::Open(p) if p.is_dir() => Some(item.title.clone()),
        _ => None,
    }
}

/// Selection movement with wrap-around. `len == 0` yields 0.
pub fn step(selected: usize, len: usize, down: bool) -> usize {
    if len == 0 {
        0
    } else if down {
        (selected + 1) % len
    } else {
        (selected + len - 1) % len
    }
}

/// Esc: clear text first, hide when already empty.
#[derive(Debug, PartialEq, Eq)]
pub enum EscOutcome {
    CancelConfirm,
    ClearText,
    Hide,
}

pub fn escape_outcome(confirming: bool, text_empty: bool) -> EscOutcome {
    if confirming {
        EscOutcome::CancelConfirm
    } else if text_empty {
        EscOutcome::Hide
    } else {
        EscOutcome::ClearText
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NONE: ModifierType = ModifierType::empty();

    #[test]
    fn navigation() {
        assert_eq!(map_key(Key::Up, NONE), Some(KeyAction::Up));
        assert_eq!(map_key(Key::Down, NONE), Some(KeyAction::Down));
        assert_eq!(map_key(Key::j, ModifierType::CONTROL_MASK), Some(KeyAction::Down));
        assert_eq!(map_key(Key::K, ModifierType::CONTROL_MASK), Some(KeyAction::Up));
        assert_eq!(map_key(Key::n, ModifierType::CONTROL_MASK), Some(KeyAction::Down));
        assert_eq!(map_key(Key::p, ModifierType::CONTROL_MASK), Some(KeyAction::Up));
    }

    #[test]
    fn running() {
        assert_eq!(map_key(Key::Return, NONE), Some(KeyAction::RunSelected(0)));
        assert_eq!(map_key(Key::KP_Enter, NONE), Some(KeyAction::RunSelected(0)));
        assert_eq!(map_key(Key::Return, ModifierType::CONTROL_MASK), Some(KeyAction::RunSelected(1)));
        assert_eq!(map_key(Key::_1, ModifierType::ALT_MASK), Some(KeyAction::RunNth(0)));
        assert_eq!(map_key(Key::_8, ModifierType::ALT_MASK), Some(KeyAction::RunNth(7)));
        assert_eq!(map_key(Key::_9, ModifierType::ALT_MASK), None);
    }

    #[test]
    fn passthrough() {
        assert_eq!(map_key(Key::a, NONE), None);
        assert_eq!(map_key(Key::a, ModifierType::CONTROL_MASK), None);
        assert_eq!(map_key(Key::_1, NONE), None);
        assert_eq!(map_key(Key::Tab, NONE), Some(KeyAction::Tab));
        assert_eq!(map_key(Key::Escape, NONE), Some(KeyAction::Escape));
    }

    #[test]
    fn tab_fills_apps_and_dirs_only() {
        let app = ResultItem::new("a", "Firefox", "apps").action(Action::LaunchApp { id: "firefox.desktop".into() });
        assert_eq!(tab_fill(&app).as_deref(), Some("Firefox"));
        let dir = ResultItem::new("d", "tmp", "files").action(Action::Open("/tmp".into()));
        assert_eq!(tab_fill(&dir).as_deref(), Some("tmp"));
        let file = ResultItem::new("f", "x", "files").action(Action::Open("/definitely/not/a/dir.txt".into()));
        assert_eq!(tab_fill(&file), None);
        let calc = ResultItem::new("c", "216", "compute").action(Action::Copy("216".into()));
        assert_eq!(tab_fill(&calc), None);
        assert_eq!(tab_fill(&ResultItem::new("e", "info", "x")), None);
    }

    #[test]
    fn stepping_wraps() {
        assert_eq!(step(0, 3, true), 1);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(0, 0, true), 0);
    }

    #[test]
    fn escape_logic() {
        assert_eq!(escape_outcome(false, false), EscOutcome::ClearText);
        assert_eq!(escape_outcome(false, true), EscOutcome::Hide);
        assert_eq!(escape_outcome(true, false), EscOutcome::CancelConfirm);
    }
}
