//! The engine log file: one line per record, timestamped, at
//! `~/Library/Logs/Detent/detent.log`. When it passes 1 MB, at launch or
//! during a long session, it becomes `detent.log.1` (replacing an older
//! one) and a new file starts, so it never grows without bound and the
//! lines before a problem survive one rotation (M9).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// A log file over this size is rotated.
pub const MAX_LEN: u64 = 1024 * 1024;

/// The usual log path, `~/Library/Logs/Detent/detent.log`.
pub fn default_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join("Library/Logs/Detent/detent.log"))
}

/// The previous log beside `path`: `detent.log` -> `detent.log.1`.
pub fn previous_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".1");
    PathBuf::from(p)
}

/// Appends lines to a file. Write errors are ignored: logging must never
/// stop the engine.
#[derive(Debug)]
pub struct FileLog {
    /// The open file and how many bytes it holds.
    file: Mutex<(File, u64)>,
    path: PathBuf,
    limit: u64,
}

impl FileLog {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        Self::with_limit(path, MAX_LEN)
    }

    /// Like [`FileLog::open`], rotating past `limit` bytes instead of 1 MB.
    pub fn with_limit(path: &Path, limit: u64) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let len = fs::metadata(path).map_or(0, |m| m.len());
        let (file, len) = if len > limit {
            (Self::rotate(path)?, 0)
        } else {
            (
                OpenOptions::new().create(true).append(true).open(path)?,
                len,
            )
        };
        Ok(Self {
            file: Mutex::new((file, len)),
            path: path.to_owned(),
            limit,
        })
    }

    /// Moves the log to `.1` and opens a new, empty one.
    fn rotate(path: &Path) -> std::io::Result<File> {
        fs::rename(path, previous_path(path))?;
        OpenOptions::new().create(true).append(true).open(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes `line` with a Unix time stamp in seconds and milliseconds,
    /// rotating first if the file has passed its limit.
    pub fn line(&self, line: &str) {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let text = format!("{}.{:03} {line}\n", t.as_secs(), t.subsec_millis());
        let mut guard = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if guard.1 > self.limit {
            if let Ok(file) = Self::rotate(&self.path) {
                *guard = (file, 0);
            }
        }
        if guard.0.write_all(text.as_bytes()).is_ok() {
            guard.1 += text.len() as u64;
        }
    }
}
