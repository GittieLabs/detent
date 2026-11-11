//! Run AppleScript through `/usr/bin/osascript` as a child process with a
//! timeout (D-053). The source goes in on stdin, so no length or quoting
//! limits apply. The child gets its own process group, and a timeout kills the
//! whole group, so a `do shell script` it started doesn't outlive it.
//!
//! The first time a script tells an app, macOS asks the user to allow it
//! (Automation) and the script waits on the answer. That wait doesn't count
//! (M9, D-106): before running, each target app's consent is checked
//! without prompting, and if it might still be asked, the run gets
//! [`PROMPT_GRACE`] on top of its timeout.

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

/// Extra time a run gets while macOS may show the Automation prompt.
pub const PROMPT_GRACE: Duration = Duration::from_secs(120);

/// An app a script sends Apple Events to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// `application "Mail"` or `app "Mail"`.
    Name(String),
    /// `application id "com.apple.mail"`.
    BundleId(String),
}

/// Whether Detent may send Apple Events to an app, as macOS says without
/// prompting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    Granted,
    Denied,
    /// The user hasn't been asked yet: this run will prompt.
    NotAsked,
    /// Can't tell (the app isn't running or isn't found).
    Unknown,
}

/// The apps `source` tells, in order of first mention, each once. A plain
/// scan for `application`/`app`, an optional `id`, then a quoted string:
/// enough for the scripts people map to a button.
pub fn targets(source: &str) -> Vec<Target> {
    let lower = source.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let word_at = |i: usize, w: &str| {
        lower[i..].starts_with(w)
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
            && bytes
                .get(i + w.len())
                .is_some_and(|b| b.is_ascii_whitespace())
    };
    let skip_space = |mut i: usize| {
        while bytes.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
            i += 1;
        }
        i
    };
    let mut found: Vec<Target> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let len = if word_at(i, "application") {
            "application".len()
        } else if word_at(i, "app") {
            "app".len()
        } else {
            i += 1;
            continue;
        };
        let mut j = skip_space(i + len);
        let by_id = word_at(j, "id");
        if by_id {
            j = skip_space(j + 2);
        }
        if bytes.get(j) == Some(&b'"') {
            if let Some(end) = source[j + 1..].find('"') {
                let text = source[j + 1..j + 1 + end].to_owned();
                let t = if by_id {
                    Target::BundleId(text)
                } else {
                    Target::Name(text)
                };
                if !text_is_empty(&t) && !found.contains(&t) {
                    found.push(t);
                }
                i = j + 1 + end + 1;
                continue;
            }
        }
        i += len;
    }
    found
}

fn text_is_empty(t: &Target) -> bool {
    match t {
        Target::Name(s) | Target::BundleId(s) => s.is_empty(),
    }
}

/// The timeout for a run whose targets have these consents: `base`, plus
/// [`PROMPT_GRACE`] if any of them might still prompt.
pub fn timeout_for(base: Duration, consents: &[Consent]) -> Duration {
    if consents
        .iter()
        .any(|c| matches!(c, Consent::NotAsked | Consent::Unknown))
    {
        base + PROMPT_GRACE
    } else {
        base
    }
}

/// Runs `source` with `run` (normally [`run`]), its timeout stretched by
/// [`timeout_for`] from each target's `consent`.
pub fn run_with_consent(
    source: &str,
    base: Duration,
    consent: impl Fn(&Target) -> Consent,
    run: impl Fn(&str, Duration) -> Result<String, ScriptError>,
) -> Result<String, ScriptError> {
    let consents: Vec<Consent> = targets(source).iter().map(consent).collect();
    run(source, timeout_for(base, &consents))
}
