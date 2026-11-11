//! The config file: versioned JSON at
//! `~/Library/Application Support/Detent/config.json` (D-008). Schema v1 is
//! read-only in M2 (hand written); writing came in M3, migrations in M9
//! (D-103): an older version is migrated step by step on load, and the old
//! file is kept beside it as `config.json.v<N>.bak`.
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
pub const SCHEMA_VERSION: u64 = 2;

/// One schema step: turns a config at version `from` into `from + 1`.
/// Steps work on the raw JSON, before it's read into [`Config`], so they
/// can rename, move or drop fields the current schema no longer knows.
pub struct Migration {
    pub from: u64,
    pub run: fn(serde_json::Value) -> Result<serde_json::Value, String>,
}

/// The steps from each earlier schema version; the frozen fixtures in
/// `tests/fixtures/configs/` prove the old shapes still load.
pub const MIGRATIONS: &[Migration] = &[Migration {
    from: 1,
    run: v1_to_v2,
}];

/// Open dashboard's default before the docs site (D-088). Detent wrote it
/// into every saved mapping that left the URL empty.
const V1_DEFAULT_DASHBOARD_URL: &str = "https://github.com/gittielabs/detent";

/// Schema 2 (D-109): an Open dashboard still on schema 1's default page,
/// the repo, shows the docs site instead. Any other URL is the user's.
fn v1_to_v2(mut v: serde_json::Value) -> Result<serde_json::Value, String> {
    let fix = |action: &mut serde_json::Value| {
        let is_old = action.get("type").and_then(|t| t.as_str()) == Some("open_dashboard")
            && action.get("url").and_then(|u| u.as_str()) == Some(V1_DEFAULT_DASHBOARD_URL);
        if is_old {
            action["url"] = DEFAULT_DASHBOARD_URL.into();
        }
    };
    if let Some(maps) = v.get_mut("mappings").and_then(|m| m.as_array_mut()) {
        for m in maps {
            if let Some(a) = m.get_mut("action") {
                fix(a);
            }
        }
    }
    if let Some(devs) = v.get_mut("devices").and_then(|d| d.as_array_mut()) {
        for d in devs {
            if let Some(items) = d.get_mut("menu").and_then(|m| m.as_array_mut()) {
                for item in items {
                    if let Some(a) = item.get_mut("action") {
                        fix(a);
                    }
                }
            }
        }
    }
    Ok(v)
}

/// Why a config couldn't be used. Every message says where the problem is.
#[derive(Debug)]
pub enum ConfigError {
    /// Not valid JSON; the message carries the line and column.
    Syntax(String),
    /// Valid JSON, wrong shape or value, at a field path.
    Invalid { path: String, message: String },
    /// A `version` this build doesn't read.
    UnsupportedVersion(u64),
    /// A migration step failed.
    Migration { from: u64, message: String },
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
            Self::Migration { from, message } => write!(
                f,
                "couldn't migrate the config from version {from} to {}: {message}",
                from + 1
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

/// Brings a raw config up to version `target` with `steps`, one version at
/// a time, and says which version it started at if it changed. A version
/// above `target`, or one with no step forward, is refused.
pub fn migrate(
    mut value: serde_json::Value,
    steps: &[Migration],
    target: u64,
) -> Result<(serde_json::Value, Option<u64>), ConfigError> {
    let version = match value.get("version") {
        None => {
            return Err(ConfigError::Invalid {
                path: String::new(),
                message: "missing field `version`".into(),
            })
        }
        Some(v) => v.as_u64().ok_or_else(|| ConfigError::Invalid {
            path: "version".into(),
            message: format!("expected a number, found {v}"),
        })?,
    };
    if version > target {
        return Err(ConfigError::UnsupportedVersion(version));
    }
    let mut at = version;
    while at < target {
        let step = steps
            .iter()
            .find(|s| s.from == at)
            .ok_or(ConfigError::UnsupportedVersion(version))?;
        value =
            (step.run)(value).map_err(|message| ConfigError::Migration { from: at, message })?;
        at += 1;
        if let Some(o) = value.as_object_mut() {
            o.insert("version".into(), at.into());
        }
    }
    Ok((value, (version < target).then_some(version)))
}

/// Parses and validates a config.
pub fn parse(json: &str) -> Result<Config, ConfigError> {
    parse_with(json, MIGRATIONS, SCHEMA_VERSION).map(|(c, _)| c)
}

/// [`parse`] with given migration steps, also saying the version it was
/// migrated from, if any.
fn parse_with(
    json: &str,
    steps: &[Migration],
    target: u64,
) -> Result<(Config, Option<u64>), ConfigError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| ConfigError::Syntax(e.to_string()))?;
    // Check the version first, so a future version gets a clear message
    // rather than whichever of its new fields fails first.
    let (value, from) = migrate(value, steps, target)?;
    let config: Config =
        serde_path_to_error::deserialize(value).map_err(|e| ConfigError::Invalid {
            path: e.path().to_string(),
            message: e.inner().to_string(),
        })?;
    validate::check(&config).map_err(|p| ConfigError::Invalid {
        path: p.path,
        message: p.message,
    })?;
    Ok((config, from))
}

/// Reads and parses the config at `path`, migrating an older version.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    load_with(path, MIGRATIONS, SCHEMA_VERSION)
}

/// [`load`] with given migration steps. When the file was migrated, a copy
/// of it as it was is kept at `<path>.v<N>.bak` (an existing backup is left
/// alone); the file itself changes only on the next save.
pub fn load_with(path: &Path, steps: &[Migration], target: u64) -> Result<Config, ConfigError> {
    let in_file = |error| ConfigError::InFile {
        path: path.to_owned(),
        error: Box::new(error),
    };
    let text = std::fs::read_to_string(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ConfigError::Missing(path.to_owned()),
        _ => ConfigError::Io {
            path: path.to_owned(),
            message: e.to_string(),
        },
    })?;
    let (config, from) = parse_with(&text, steps, target).map_err(in_file)?;
    if let Some(from) = from {
        let mut backup = path.as_os_str().to_owned();
        backup.push(format!(".v{from}.bak"));
        let backup = PathBuf::from(backup);
        if !backup.exists() {
            std::fs::write(&backup, &text).map_err(|e| ConfigError::Io {
                path: backup.clone(),
                message: e.to_string(),
            })?;
        }
    }
    Ok(config)
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
