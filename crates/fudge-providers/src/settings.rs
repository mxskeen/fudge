//! `set bluetooth`: jump to a system settings panel, mapped per desktop.

use fudge_core::fuzzy;
use fudge_core::{Action, DesktopKind, Icon, Provider, Query, ResultItem};

pub struct Panel {
    pub key: &'static str,
    pub title: &'static str,
    pub aliases: &'static [&'static str],
    /// GNOME Control Center panel name ("" = none).
    pub gnome: &'static str,
    /// KDE KCM module for `systemsettings` ("" = none).
    pub kde: &'static str,
    /// `x-apple.systempreferences:` URL ("" = none).
    pub mac: &'static str,
    pub icon: &'static str,
}


macro_rules! mac {
    ($s:literal) => {
        concat!("x-apple.systempreferences:com.apple.", $s)
    };
}

pub const PANELS: &[Panel] = &[
    Panel { key: "wifi", title: "Wi-Fi", aliases: &["wifi", "wi-fi", "wireless"], gnome: "wifi", kde: "kcm_networkmanagement", mac: mac!("wifi-settings-extension"), icon: "network-wireless" },
    Panel { key: "bluetooth", title: "Bluetooth", aliases: &["bluetooth"], gnome: "bluetooth", kde: "kcm_bluetooth", mac: mac!("BluetoothSettings"), icon: "bluetooth" },
    Panel { key: "display", title: "Displays", aliases: &["display", "displays", "monitor", "screen", "resolution"], gnome: "display", kde: "kcm_kscreen", mac: mac!("Displays-Settings.extension"), icon: "preferences-desktop-display" },
    Panel { key: "sound", title: "Sound", aliases: &["sound", "audio", "volume"], gnome: "sound", kde: "kcm_pulseaudio", mac: mac!("Sound-Settings.extension"), icon: "audio-volume-high" },
    Panel { key: "power", title: "Power", aliases: &["power", "battery", "energy"], gnome: "power", kde: "kcm_powerdevilprofilesconfig", mac: mac!("Battery-Settings.extension"), icon: "battery" },
    Panel { key: "network", title: "Network", aliases: &["network", "ethernet", "vpn", "proxy"], gnome: "network", kde: "kcm_networkmanagement", mac: mac!("Network-Settings.extension"), icon: "preferences-system-network" },
    Panel { key: "keyboard", title: "Keyboard", aliases: &["keyboard", "shortcuts", "input"], gnome: "keyboard", kde: "kcm_keyboard", mac: mac!("Keyboard-Settings.extension"), icon: "input-keyboard" },
    Panel { key: "mouse", title: "Mouse", aliases: &["mouse", "touchpad", "trackpad"], gnome: "mouse", kde: "kcm_mouse", mac: mac!("Mouse-Settings.extension"), icon: "input-mouse" },
    Panel { key: "printers", title: "Printers", aliases: &["printers", "printer", "printing", "scanner"], gnome: "printers", kde: "kcm_printer_manager", mac: mac!("Print-Scan-Settings.extension"), icon: "printer" },
    Panel { key: "users", title: "Users", aliases: &["users", "accounts", "user accounts"], gnome: "system/users", kde: "kcm_users", mac: mac!("Users-Groups-Settings.extension"), icon: "system-users" },
    Panel { key: "datetime", title: "Date & Time", aliases: &["date & time", "date and time", "time", "clock", "timezone"], gnome: "system/datetime", kde: "kcm_clock", mac: mac!("Date-Time-Settings.extension"), icon: "preferences-system-time" },
    Panel { key: "notifications", title: "Notifications", aliases: &["notifications", "alerts"], gnome: "notifications", kde: "kcm_notifications", mac: mac!("Notifications-Settings.extension"), icon: "preferences-system-notifications" },
    Panel { key: "privacy", title: "Privacy", aliases: &["privacy", "security", "location"], gnome: "privacy", kde: "kcm_privacy", mac: mac!("settings.PrivacySecurity.extension"), icon: "preferences-system-privacy" },
    Panel { key: "appearance", title: "Appearance", aliases: &["appearance", "theme", "dark mode", "style"], gnome: "background", kde: "kcm_lookandfeel", mac: mac!("Appearance-Settings.extension"), icon: "preferences-desktop-theme" },
    Panel { key: "background", title: "Wallpaper", aliases: &["wallpaper", "background"], gnome: "background", kde: "kcm_wallpaper", mac: mac!("Wallpaper-Settings.extension"), icon: "preferences-desktop-wallpaper" },
    Panel { key: "apps", title: "Default Apps", aliases: &["apps", "applications", "default apps", "startup"], gnome: "applications", kde: "kcm_componentchooser", mac: mac!("LoginItems-Settings.extension"), icon: "preferences-desktop-default-applications" },
    Panel { key: "about", title: "About", aliases: &["about", "system info", "device info"], gnome: "system/about", kde: "kcm_about-distro", mac: mac!("SystemProfiler.AboutExtension"), icon: "help-about" },
    Panel { key: "region", title: "Language & Region", aliases: &["region", "language", "locale", "formats"], gnome: "system/region", kde: "kcm_regionandlang", mac: mac!("Localization-Settings.extension"), icon: "preferences-desktop-locale" },
    Panel { key: "accessibility", title: "Accessibility", aliases: &["accessibility", "a11y", "universal access"], gnome: "universal-access", kde: "kcm_access", mac: mac!("Accessibility-Settings.extension"), icon: "preferences-desktop-accessibility" },
    Panel { key: "sharing", title: "Sharing", aliases: &["sharing", "remote desktop"], gnome: "sharing", kde: "", mac: mac!("Sharing-Settings.extension"), icon: "preferences-system-sharing" },
    Panel { key: "online-accounts", title: "Online Accounts", aliases: &["online accounts", "cloud accounts"], gnome: "online-accounts", kde: "kcm_kaccounts", mac: mac!("Internet-Accounts-Settings.extension"), icon: "preferences-system-online-accounts" },
    Panel { key: "multitasking", title: "Multitasking", aliases: &["multitasking", "workspaces", "hot corner"], gnome: "multitasking", kde: "kcm_kwin_virtualdesktops", mac: "x-apple.systempreferences:com.apple.Desktop-Settings.extension", icon: "preferences-system-windows" },
    Panel { key: "color", title: "Color", aliases: &["color", "colour", "color profile"], gnome: "color", kde: "kcm_colors", mac: mac!("Displays-Settings.extension"), icon: "preferences-color" },
    Panel { key: "search", title: "Search", aliases: &["search", "indexing"], gnome: "search", kde: "kcm_plasmasearch", mac: mac!("Siri-Settings.extension"), icon: "preferences-system-search" },
    Panel { key: "screensaver", title: "Screen Lock", aliases: &["screen lock", "lock screen", "screensaver"], gnome: "privacy/screen", kde: "kcm_screenlocker", mac: mac!("ScreenSaver-Settings.extension"), icon: "system-lock-screen" },
    Panel { key: "general", title: "System Settings", aliases: &["settings", "system settings", "preferences"], gnome: "info-overview", kde: "", mac: "x-apple.systempreferences:", icon: "preferences-system" },
];

/// The string `Platform::open_settings_panel` should receive on this desktop.
pub fn resolve(panel: &Panel, desktop: DesktopKind) -> &'static str {
    match desktop {
        DesktopKind::MacOs => panel.mac,
        DesktopKind::Kde => panel.kde,
        _ => panel.gnome,
    }
}

pub struct SettingsProvider {
    desktop: DesktopKind,
    keywords: Vec<&'static str>,
}

impl SettingsProvider {
    pub fn new(desktop: DesktopKind) -> Self {
        Self { desktop, keywords: vec!["set"] }
    }

    fn item(&self, p: &Panel, target: &'static str, score: f32) -> ResultItem {
        ResultItem::new(format!("settings:{}", p.key), format!("{} settings", p.title), "settings")
            .subtitle("Open system settings")
            .icon(Icon::Named(p.icon.into()))
            .score(score)
            .action(Action::OpenSettingsPanel(target.into()))
    }
}

impl Provider for SettingsProvider {
    fn id(&self) -> &'static str {
        "settings"
    }

    fn keywords(&self) -> &[&'static str] {
        &self.keywords
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        let explicit = q.keyword.as_deref() == Some("set");
        if q.keyword.is_some() && !explicit {
            return Vec::new();
        }
        let text = if explicit { q.rest.as_str() } else { q.text.as_str() };
        if !explicit && text.chars().count() < 3 {
            return Vec::new();
        }
        let factor = if explicit { 1.0 } else { 0.6 };
        let mut out: Vec<ResultItem> = PANELS
            .iter()
            .filter_map(|p| {
                let target = resolve(p, self.desktop);
                if target.is_empty() {
                    return None;
                }
                if text.is_empty() {
                    return Some(self.item(p, target, 50.0));
                }
                let s = fuzzy::best(text, p.aliases.iter().copied().chain([p.title]))?;
                // Unprefixed queries must be a clear match so "set" noise never floods results.
                if !explicit && s < 70.0 {
                    return None;
                }
                Some(self.item(p, target, s * factor))
            })
            .collect();
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(8);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kw(s: &str) -> bool {
        s == "set"
    }

    #[test]
    fn explicit_keyword() {
        let p = SettingsProvider::new(DesktopKind::Gnome);
        let r = p.query(&Query::parse("set blue", kw));
        assert_eq!(r[0].title, "Bluetooth settings");
        assert_eq!(r[0].actions, vec![Action::OpenSettingsPanel("bluetooth".into())]);
        assert!(p.query(&Query::parse("set", kw)).len() >= 8);
    }

    #[test]
    fn per_desktop_mapping() {
        let kde = SettingsProvider::new(DesktopKind::Kde);
        let r = kde.query(&Query::parse("set bluetooth", kw));
        assert_eq!(r[0].actions, vec![Action::OpenSettingsPanel("kcm_bluetooth".into())]);
        let mac = SettingsProvider::new(DesktopKind::MacOs);
        let r = mac.query(&Query::parse("set bluetooth", kw));
        assert_eq!(r[0].actions, vec![Action::OpenSettingsPanel("x-apple.systempreferences:com.apple.BluetoothSettings".into())]);
    }

    #[test]
    fn unprefixed_is_lower_and_stricter() {
        let p = SettingsProvider::new(DesktopKind::Gnome);
        let a = p.query(&Query::parse("bluetooth", kw));
        let b = p.query(&Query::parse("set bluetooth", kw));
        assert_eq!(a[0].title, "Bluetooth settings");
        assert!(a[0].score < b[0].score);
        assert!(p.query(&Query::parse("bl", kw)).is_empty());
        assert!(p.query(&Query::parse("firefox", kw)).is_empty());
    }

    #[test]
    fn has_about_25_panels() {
        assert!(PANELS.len() >= 25);
    }
}
