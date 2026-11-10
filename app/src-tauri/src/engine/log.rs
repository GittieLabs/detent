//! The engine log file: one line per record, timestamped, at
//! `~/Library/Logs/Detent/detent.log`. Started fresh when it passes 1 MB at
//! launch, so it never grows without bound.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// A log file over this size at launch is started over.
pub const MAX_LEN: u64 = 1024 * 1024;

/// The usual log path, `~/Library/Logs/Detent/detent.log`.
pub fn default_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join("Library/Logs/Detent/detent.log"))
}

/// Appends lines to a file. Write errors are ignored: logging must never
/// stop the engine.
#[derive(Debug)]
pub struct FileLog {
    file: Mutex<File>,
    path: PathBuf,
}

impl FileLog {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let too_big = fs::metadata(path).is_ok_and(|m| m.len() > MAX_LEN);
        let file = OpenOptions::new()
            .create(true)
            .append(!too_big)
            .write(true)
            .truncate(too_big)
            .open(path)?;
        Ok(Self {
            file: Mutex::new(file),
            path: path.to_owned(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes `line` with a Unix time stamp in seconds and milliseconds.
    pub fn line(&self, line: &str) {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let mut f = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(f, "{}.{:03} {line}", t.as_secs(), t.subsec_millis());
    }
}
