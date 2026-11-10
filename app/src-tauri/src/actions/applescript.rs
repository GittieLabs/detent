//! Run AppleScript through `/usr/bin/osascript` as a child process with a
//! timeout (D-053). The source goes in on stdin, so no length or quoting
//! limits apply. The child gets its own process group, and a timeout kills the
//! whole group, so a `do shell script` it started doesn't outlive it.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const OSASCRIPT: &str = "/usr/bin/osascript";
/// How often the child is checked for exit. Small next to any script's run time.
const POLL: Duration = Duration::from_millis(5);

/// Why a script didn't produce a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// `osascript` couldn't be started.
    Spawn(String),
    /// The script ran past its timeout and was stopped.
    TimedOut { timeout_ms: u128 },
    /// The script failed to compile or raised an error. `message` is
    /// osascript's stderr, which includes the AppleScript error text.
    Failed { code: Option<i32>, message: String },
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "couldn't start osascript: {e}"),
            Self::TimedOut { timeout_ms } => {
                write!(
                    f,
                    "AppleScript timed out after {timeout_ms} ms and was stopped"
                )
            }
            Self::Failed { message, .. } => write!(f, "AppleScript failed: {message}"),
        }
    }
}

impl std::error::Error for ScriptError {}

fn drain(mut pipe: impl Read + Send + 'static) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    })
}

/// Stops the child and everything in its process group, then reaps it.
fn stop(child: &mut Child) {
    // No libc outside `macos/` (no `unsafe`), so signal the group with kill(1).
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs `source` and returns its result as osascript prints it, trimmed.
/// Blocks the calling thread for at most about `timeout`.
pub fn run(source: &str, timeout: Duration) -> Result<String, ScriptError> {
    let mut child = Command::new(OSASCRIPT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| ScriptError::Spawn(e.to_string()))?;

    let mut stdin = child.stdin.take().expect("stdin is piped");
    let source = source.to_owned();
    // Written from a thread so a large script can't block on a full pipe.
    thread::spawn(move || {
        let _ = stdin.write_all(source.as_bytes());
        // Dropping stdin closes it: osascript then compiles and runs.
    });
    let stdout = drain(child.stdout.take().expect("stdout is piped"));
    let stderr = drain(child.stderr.take().expect("stderr is piped"));

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                stop(&mut child);
                // The reader threads finish on their own once the pipes close.
                return Err(ScriptError::TimedOut {
                    timeout_ms: timeout.as_millis(),
                });
            }
            Ok(None) => thread::sleep(POLL),
            Err(e) => {
                stop(&mut child);
                return Err(ScriptError::Spawn(e.to_string()));
            }
        }
    };

    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();
    if status.success() {
        Ok(out.trim_end_matches('\n').to_owned())
    } else {
        Err(ScriptError::Failed {
            code: status.code(),
            message: err.trim().to_owned(),
        })
    }
}
