//! The config file: versioned JSON at
//! `~/Library/Application Support/Detent/config.json` (D-008). Schema v1 is
//! read-only in M2 (hand written); writing and migrations come in M3 and M7.
//! Field reference: `app/src-tauri/examples/config.md`.

mod schema;
mod validate;

use std::fmt;
use std::path::{Path, PathBuf};

pub use schema::{
    Action, Config, DashboardMode, DeviceEntry, Mapping, MenuItem, Scope, Trigger, UnitRef,
    DEFAULT_APPLESCRIPT_TIMEOUT_MS, DEFAULT_DASHBOARD_URL, DEFAULT_VOLUME_STEP, MAX_MENU_ITEMS,
};
pub use validate::ArgError;

/// The schema version this build reads.
pub const SCHEMA_VERSION: u64 = 1;

/// Why a config couldn't be used. Every message says where the problem is.
#[derive(Debug)]
pub enum ConfigError {
    /// Not valid JSON; the message carries the line and column.
    Syntax(String),
    /// Valid JSON, wrong shape or value, at a field path.
    Invalid { path: String, message: String },
    /// A `version` this build doesn't read.
    UnsupportedVersion(u64),
    /// No file at the path.
    Missing(PathBuf),
    /// The file couldn't be read.
    Io { path: PathBuf, message: String },
    /// One of the above, in a particular file.
    InFile {
        path: PathBuf,
        error: Box<ConfigError>,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(m) => write!(f, "config isn't valid JSON: {m}"),
            Self::Invalid { path, message } if path.is_empty() || path == "." => {
                write!(f, "{message}")
            }
            Self::Invalid { path, message } => write!(f, "{path}: {message}"),
            Self::UnsupportedVersion(v) => write!(
                f,
                "unsupported config version {v} (this Detent reads version {SCHEMA_VERSION})"
            ),
            Self::Missing(p) => write!(
                f,
                "no config at {}; copy app/src-tauri/examples/config.example.json there to start",
                p.display()
            ),
            Self::Io { path, message } => write!(f, "couldn't read {}: {message}", path.display()),
            Self::InFile { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Parses and validates a config.
pub fn parse(json: &str) -> Result<Config, ConfigError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| ConfigError::Syntax(e.to_string()))?;
    // Check the version first, so a future version gets a clear message
    // rather than whichever of its new fields fails first.
    match value.get("version") {
        None => {
            return Err(ConfigError::Invalid {
                path: String::new(),
                message: "missing field `version`".into(),
            })
        }
        Some(v) if v.as_u64() != Some(SCHEMA_VERSION) => {
            return Err(match v.as_u64() {
                Some(n) => ConfigError::UnsupportedVersion(n),
                None => ConfigError::Invalid {
                    path: "version".into(),
                    message: format!("expected a number, found {v}"),
                },
            })
        }
        Some(_) => {}
    }
    let config: Config =
        serde_path_to_error::deserialize(value).map_err(|e| ConfigError::Invalid {
            path: e.path().to_string(),
            message: e.inner().to_string(),
        })?;
    validate::check(&config).map_err(|p| ConfigError::Invalid {
        path: p.path,
        message: p.message,
    })?;
    Ok(config)
}

/// Reads and parses the config at `path`.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ConfigError::Missing(path.to_owned()),
        _ => ConfigError::Io {
            path: path.to_owned(),
            message: e.to_string(),
        },
    })?;
    parse(&text).map_err(|e| ConfigError::InFile {
        path: path.to_owned(),
        error: Box::new(e),
    })
}

/// The config as Detent writes it (D-069): validated first, then pretty
/// printed in schema order with two space indents, USB IDs in hex, and
/// every default written out. Saving the result again gives the same bytes.
pub fn to_json(config: &Config) -> Result<String, ConfigError> {
    validate::check(config).map_err(|p| ConfigError::Invalid {
        path: p.path,
        message: p.message,
    })?;
    let mut text = serde_json::to_string_pretty(config).map_err(|e| ConfigError::Invalid {
        path: String::new(),
        message: e.to_string(),
    })?;
    text.push('\n');
    Ok(text)
}

/// Writes `config` to `path` atomically: to `<path>.tmp` in the same folder,
/// flushed to disk, then renamed over the old file. A failure at any point
/// leaves the old file as it was. Creates the folder if needed. An invalid
/// config is refused before anything is written.
pub fn save(path: &Path, config: &Config) -> Result<(), ConfigError> {
    use std::io::Write;
    let text = to_json(config)?;
    let io = |e: std::io::Error| ConfigError::Io {
        path: path.to_owned(),
        message: e.to_string(),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let written = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() && tmp.is_file() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.map_err(io)
}

/// Moves a config that won't load out of the way, to
/// `<path>.bad-<unix_secs>` beside it (`-2`, `-3`, ... if that's taken), so
/// Start fresh never destroys what the user wrote. Returns the new path.
pub fn move_aside(path: &Path, unix_secs: u64) -> Result<PathBuf, ConfigError> {
    let io = |e: std::io::Error| ConfigError::Io {
        path: path.to_owned(),
        message: e.to_string(),
    };
    if !path.is_file() {
        return Err(ConfigError::Missing(path.to_owned()));
    }
    let name = |n: u32| {
        let mut p = path.as_os_str().to_owned();
        p.push(format!(".bad-{unix_secs}"));
        if n > 1 {
            p.push(format!("-{n}"));
        }
        PathBuf::from(p)
    };
    let to = (1..)
        .map(name)
        .find(|p| !p.exists())
        .expect("some suffix is free");
    std::fs::rename(path, &to).map_err(io)?;
    Ok(to)
}

/// `~/Library/Application Support/Detent/config.json`, or `None` without a
/// home directory.
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join("Library/Application Support/Detent")
            .join("config.json")
    })
}
