//! Where Truffle keeps its files: `~/Library/Application Support/Truffle` on macOS, `$XDG_DATA_HOME/truffle` (or
//! `~/.local/share/truffle`) on Linux, or `TRUFFLE_DATA`. The name index lives at the root; the meaning index in
//! `semantic/`, the model in `models/`.

use std::path::PathBuf;

/// The user's home folder.
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// The data root: the model, the indexes, the logs and the settings.
pub fn data_root() -> PathBuf {
    if let Some(chosen) = std::env::var_os("TRUFFLE_DATA").filter(|value| !value.is_empty()) {
        return PathBuf::from(chosen);
    }
    let home = home().unwrap_or_else(|| PathBuf::from("/"));
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/Truffle");
    }
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map_or_else(|| home.join(".local/share"), PathBuf::from)
        .join("truffle")
}

pub fn models() -> PathBuf {
    data_root().join("models")
}

pub fn index() -> PathBuf {
    data_root().join("semantic")
}

pub fn logs() -> PathBuf {
    data_root().join("logs")
}

pub fn settings_file() -> PathBuf {
    data_root().join("settings.json")
}

pub const MAIN_INDEX_FILE: &str = "index.sqlite";
pub const CODE_INDEX_FILE: &str = "code.sqlite";
