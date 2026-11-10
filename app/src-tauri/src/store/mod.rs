//! The config file: versioned JSON at
//! `~/Library/Application Support/Detent/config.json` (D-008). Schema v1 is
//! read-only in M2 (hand written); writing and migrations come in M3 and M7.
//! Field reference: `app/src-tauri/examples/config.md`.

mod schema;
mod validate;

use std::fmt;
use std::path::{Path, PathBuf};

pub use schema::{
    Action, Config, DeviceEntry, Mapping, Scope, Trigger, UnitRef, DEFAULT_APPLESCRIPT_TIMEOUT_MS,
    DEFAULT_VOLUME_STEP,
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

/// `~/Library/Application Support/Detent/config.json`, or `None` without a
/// home directory.
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join("Library/Application Support/Detent")
            .join("config.json")
    })
}
