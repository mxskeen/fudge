//! `install-autostart` and `install-shortcut` (GNOME custom keybinding via gsettings).

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::hotkey::Hotkey;

const MK: &str = "org.gnome.settings-daemon.plugins.media-keys";
const KEY_PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/fudge/";

pub fn autostart_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart").join("fudge.desktop"))
}

pub fn autostart_contents(exe: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Fudge Launcher\nComment=Alt+Space launcher\n\
Exec={exe} daemon\nIcon=fudge\nTerminal=false\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n"
    )
}

pub fn install_autostart() -> Result<()> {
    let exe = std::env::current_exe().context("cannot find own path")?;
    let path = autostart_path().context("no config directory")?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, autostart_contents(&exe.display().to_string()))?;
    println!("Wrote {}", path.display());
    Ok(())
}

/// Parse the output of `gsettings get ... custom-keybindings`, e.g. `['/a/', '/b/']` or `@as []`.
pub fn parse_gvariant_strv(s: &str) -> Vec<String> {
    s.split('\'').skip(1).step_by(2).map(str::to_string).collect()
}

pub fn format_gvariant_strv(v: &[String]) -> String {
    let items: Vec<String> = v.iter().map(|p| format!("'{p}'")).collect();
    format!("[{}]", items.join(", "))
}

fn gsettings(args: &[&str]) -> Result<String> {
    let out = Command::new("gsettings")
        .args(args)
        .output()
        .context("cannot run gsettings (is this GNOME?)")?;
    if !out.status.success() {
        bail!("gsettings {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn install_shortcut(hotkey: &str) -> Result<()> {
    let hk = Hotkey::parse(hotkey).map_err(|e| anyhow::anyhow!("{e}"))?;
    let exe = std::env::current_exe().context("cannot find own path")?;
    let mut list = parse_gvariant_strv(&gsettings(&["get", MK, "custom-keybindings"])?);
    if !list.iter().any(|p| p == KEY_PATH) {
        list.push(KEY_PATH.to_string());
        gsettings(&["set", MK, "custom-keybindings", &format_gvariant_strv(&list)])?;
    }
    let schema = format!("{MK}.custom-keybinding:{KEY_PATH}");
    gsettings(&["set", &schema, "name", "Fudge Launcher"])?;
    gsettings(&["set", &schema, "command", &format!("{} toggle", exe.display())])?;
    gsettings(&["set", &schema, "binding", &hk.to_gnome_accel()])?;
    println!("Registered GNOME shortcut {} -> `{} toggle`.", hk.to_gnome_accel(), exe.display());
    if hk.to_gnome_accel() == "<Alt>space" {
        println!(
            "\nNote: Alt+Space is GNOME's default 'window menu' shortcut and will conflict.\n\
Unset it with:\n  gsettings set org.gnome.desktop.wm.keybindings activate-window-menu \"[]\""
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gvariant_lists() {
        assert!(parse_gvariant_strv("@as []").is_empty());
        assert_eq!(parse_gvariant_strv("['/a/', '/b/c/']"), vec!["/a/", "/b/c/"]);
        assert_eq!(format_gvariant_strv(&["/a/".into(), "/b/".into()]), "['/a/', '/b/']");
        assert_eq!(format_gvariant_strv(&[]), "[]");
    }

    #[test]
    fn desktop_entry() {
        let c = autostart_contents("/usr/bin/fudge");
        assert!(c.contains("Exec=/usr/bin/fudge daemon"));
    }
}
