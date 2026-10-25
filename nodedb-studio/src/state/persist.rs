//! On-disk storage for personal Studio state: preferences and workspace
//! layout. Never database state, never credentials.
//!
//! Files live in the per-user config directory:
//! - Linux: `$XDG_CONFIG_HOME/nodedb-studio`, else `~/.config/nodedb-studio`
//! - macOS: `~/Library/Application Support/nodedb-studio`
//! - Windows: `%APPDATA%\nodedb-studio`
//!
//! A missing file reads as the default. An unreadable or invalid file also
//! reads as the default, and the error goes to stderr: a broken layout file
//! must never stop Studio from starting.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

const APP_DIR: &str = "nodedb-studio";

/// Whether this run reads and writes the files. The screenshot run turns it
/// off, so captures never depend on, or overwrite, the user's own layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistMode {
    On,
    Off,
}

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("no per-user config directory: set HOME (or APPDATA on Windows)")]
    NoConfigDir,
    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot parse {}: {source}. Delete the file to reset it.", path.display())]
    Parse {
        path: PathBuf,
        source: sonic_rs::Error,
    },
    #[error("cannot serialize {name}: {source}")]
    Serialize {
        name: &'static str,
        source: sonic_rs::Error,
    },
    #[error("cannot write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// One stored document, e.g. `workspace.json`.
#[derive(Debug, Clone, Copy)]
pub struct Store {
    name: &'static str,
}

pub const WORKSPACE: Store = Store {
    name: "workspace.json",
};
pub const PREFERENCES: Store = Store {
    name: "preferences.json",
};

fn config_dir() -> Result<PathBuf, PersistError> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let base = if cfg!(target_os = "windows") {
        var("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join(APP_DIR))
        .ok_or(PersistError::NoConfigDir)
}

impl Store {
    fn path(self) -> Result<PathBuf, PersistError> {
        Ok(config_dir()?.join(self.name))
    }

    /// The stored value, or the default when the file is absent or invalid.
    pub fn load<T: DeserializeOwned + Default>(self, mode: PersistMode) -> T {
        if mode == PersistMode::Off {
            return T::default();
        }
        match self.path().and_then(|p| read_from(&p)) {
            Ok(Some(v)) => v,
            Ok(None) => T::default(),
            Err(e) => {
                eprintln!("nodedb-studio: {e}");
                T::default()
            }
        }
    }

    /// Writes the value on a background thread, so the UI never waits on
    /// the disk. Errors go to stderr.
    pub fn save<T: Serialize>(self, mode: PersistMode, value: &T) {
        if mode == PersistMode::Off {
            return;
        }
        let json = match sonic_rs::to_string_pretty(value) {
            Ok(j) => j,
            Err(source) => {
                let e = PersistError::Serialize {
                    name: self.name,
                    source,
                };
                eprintln!("nodedb-studio: {e}");
                return;
            }
        };
        std::thread::spawn(move || {
            if let Err(e) = self.path().and_then(|p| write_to(&p, &json)) {
                eprintln!("nodedb-studio: {e}");
            }
        });
    }
}

fn read_from<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, PersistError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PersistError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    sonic_rs::from_str(&text)
        .map(Some)
        .map_err(|source| PersistError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

/// Writes through a temporary file and a rename, so a crash mid-write never
/// leaves a truncated file behind.
fn write_to(path: &Path, json: &str) -> Result<(), PersistError> {
    let write_err = |source| PersistError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(write_err)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(write_err)?;
    std::fs::rename(&tmp, path).map_err(write_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::workspace::Workspace;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("nodedb-studio-persist-{}", std::process::id()))
            .join(name)
    }

    #[test]
    fn missing_file_reads_as_none() {
        let got: Option<Workspace> = read_from(&scratch("absent.json")).expect("not an error");
        assert!(got.is_none());
    }

    #[test]
    fn written_value_reads_back() {
        let path = scratch("round-trip.json");
        let mut ws = Workspace::default();
        ws.set_nav_collapsed(true);
        let json = sonic_rs::to_string(&ws).expect("serializes");
        write_to(&path, &json).expect("writes");
        let got: Option<Workspace> = read_from(&path).expect("reads");
        assert_eq!(got, Some(ws));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_json_is_a_parse_error_naming_the_file() {
        let path = scratch("broken.json");
        write_to(&path, "{not json").expect("writes");
        let err = read_from::<Workspace>(&path).expect_err("must not parse");
        assert!(err.to_string().contains("broken.json"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn off_mode_never_reads() {
        let ws: Workspace = WORKSPACE.load(PersistMode::Off);
        assert_eq!(ws, Workspace::default());
    }
}
