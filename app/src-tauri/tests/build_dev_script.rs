//! Failure paths of `scripts/build-dev.sh`. These stop before any build runs,
//! so they're fast. The signed build itself is a manual step (acceptance.md).

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::{Command, Output};

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/build-dev.sh")
}

fn run(identity: Option<&str>) -> Output {
    let mut cmd = Command::new(script());
    match identity {
        Some(id) => cmd.env("APPLE_SIGNING_IDENTITY", id),
        None => cmd.env_remove("APPLE_SIGNING_IDENTITY"),
    };
    cmd.output().expect("scripts/build-dev.sh runs")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn fails_clearly_when_identity_is_unset() {
    let out = run(None);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("APPLE_SIGNING_IDENTITY is not set"), "{err}");
    assert!(err.contains("security find-identity"), "{err}");
}

#[test]
fn fails_clearly_when_identity_is_empty() {
    let out = run(Some(""));
    assert!(!out.status.success());
    assert!(stderr(&out).contains("APPLE_SIGNING_IDENTITY is not set"));
}

#[test]
fn rejects_ad_hoc_signing() {
    let out = run(Some("-"));
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("ad hoc"), "{err}");
    assert!(err.contains("D-010"), "{err}");
}

#[test]
fn rejects_an_identity_missing_from_the_keychain() {
    let out = run(Some("Apple Development: Nobody Here (ZZZZZZZZZZ)"));
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("no valid codesigning identity"), "{err}");
}
