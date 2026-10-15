//! The control channel's key file, one rule for the app (which writes it) and the MCP bridge
//! (which reads it): `<settings>/control-key.<instance>`. See `docs/control-protocol.md`, Keys.

use std::path::{Path, PathBuf};

/// The app's settings folder (preferences, logs, key files): macOS
/// `~/Library/Application Support/NoteCraft`, Windows `%APPDATA%\NoteCraft`, otherwise
/// `$XDG_CONFIG_HOME/notecraft` or `~/.config/notecraft`.
pub fn settings_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/NoteCraft"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("NoteCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("notecraft"))
    }
}

/// The app's data folder (backups): macOS `~/Library/Application Support/NoteCraft`, Windows
/// `%LOCALAPPDATA%\NoteCraft`, otherwise `$XDG_DATA_HOME/notecraft` or
/// `~/.local/share/notecraft`.
pub fn data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        settings_dir()
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|a| PathBuf::from(a).join("NoteCraft"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|c| c.join("notecraft"))
    }
}

/// The `instance-id=` value of a `/.flatpak-info` file, only when it is `[A-Za-z0-9_-]+` (it
/// becomes part of a file name).
pub fn parse_instance_id(info: &str) -> Option<String> {
    let v = info.lines().find_map(|l| l.strip_prefix("instance-id="))?.trim();
    (!v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')).then(|| v.to_string())
}

/// The Flatpak instance id, or `None` outside Flatpak.
pub fn instance_id() -> Option<String> {
    parse_instance_id(&std::fs::read_to_string("/.flatpak-info").ok()?)
}

/// `<settings>/control-key.<instance>`, with `<instance>` the Flatpak instance id or
/// `local-<port>`.
pub fn key_path(settings: &Path, port: u16) -> PathBuf {
    key_path_for(settings, instance_id().as_deref(), port)
}

/// [`key_path`] with the instance given.
pub fn key_path_for(settings: &Path, instance: Option<&str>, port: u16) -> PathBuf {
    match instance {
        Some(id) => settings.join(format!("control-key.{id}")),
        None => settings.join(format!("control-key.local-{port}")),
    }
}

/// The control key for `port`: `env` (`NOTECRAFT_CONTROL_KEY`) when set, else the app's key
/// file in `settings`. The error says where it looked.
pub fn lookup_key(env: Option<String>, settings: Option<&Path>, port: Option<u16>) -> Result<String, String> {
    if let Some(k) = env.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        return Ok(k);
    }
    let (Some(settings), Some(port)) = (settings, port) else {
        return Err("no control key: set NOTECRAFT_CONTROL_KEY (no settings folder or port to find the key file)".to_string());
    };
    let path = key_path(settings, port);
    match std::fs::read_to_string(&path).map(|k| k.trim().to_string()) {
        Ok(k) if !k.is_empty() => Ok(k),
        _ => Err(format!("no control key: set NOTECRAFT_CONTROL_KEY, or start the app with --control {port} (it writes {})", path.display())),
    }
}
