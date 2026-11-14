use std::path::PathBuf;

fn app_dir_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Fudge"
    } else {
        "fudge"
    }
}

/// `$XDG_DATA_HOME/fudge` or `~/Library/Application Support/Fudge`.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(app_dir_name()))
}

/// `$XDG_CONFIG_HOME/fudge` or `~/Library/Application Support/Fudge`.
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(app_dir_name()))
}
