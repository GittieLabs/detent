//! Failure paths of `scripts/build-release.sh` (D-099). These stop before any
//! build or upload runs, so they're fast. The signed, notarized build itself
//! is a manual step (acceptance.md M9).

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::{Command, Output};

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/build-release.sh")
}

fn run(args: &[&str], identity: Option<&str>, profile: Option<&str>) -> Output {
    let mut cmd = Command::new(script());
    cmd.args(args);
    match identity {
        Some(id) => cmd.env("APPLE_SIGNING_IDENTITY", id),
        None => cmd.env_remove("APPLE_SIGNING_IDENTITY"),
    };
    match profile {
        Some(p) => cmd.env("APPLE_NOTARY_PROFILE", p),
        None => cmd.env_remove("APPLE_NOTARY_PROFILE"),
    };
    cmd.output().expect("scripts/build-release.sh runs")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn fails_clearly_when_identity_is_unset() {
    let out = run(&[], None, None);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("APPLE_SIGNING_IDENTITY is not set"), "{err}");
    assert!(err.contains("Developer ID Application"), "{err}");
}

#[test]
fn refuses_a_development_identity() {
    let out = run(
        &[],
        Some("Apple Development: Nobody Here (ZZZZZZZZZZ)"),
        None,
    );
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("needs a Developer ID Application identity"),
        "{err}"
    );
}

#[test]
fn rejects_ad_hoc_signing() {
    let out = run(&[], Some("-"), None);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("needs a Developer ID Application identity"));
}

#[test]
fn rejects_an_identity_missing_from_the_keychain() {
    let out = run(
        &[],
        Some("Developer ID Application: Nobody Here (ZZZZZZZZZZ)"),
        None,
    );
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("no valid codesigning identity"), "{err}");
}

#[test]
fn rejects_a_hash_missing_from_the_keychain() {
    let out = run(&[], Some("00000000000000000000000000000000000000AA"), None);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains(
            "no valid codesigning identity with hash 00000000000000000000000000000000000000AA"
        ),
        "{err}"
    );
}

#[test]
fn notarize_needs_a_profile() {
    let out = run(&["--notarize"], None, None);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("APPLE_NOTARY_PROFILE is not set"), "{err}");
    assert!(err.contains("notarytool store-credentials"), "{err}");
}

#[test]
fn notarize_needs_a_built_dmg() {
    let out = run(
        &["--notarize", "--dmg", "/nonexistent/Detent.dmg"],
        None,
        Some("detent-test-profile"),
    );
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("no DMG at /nonexistent/Detent.dmg"), "{err}");
    assert!(err.contains("just build-release"), "{err}");
}

#[test]
fn rejects_unknown_arguments() {
    let out = run(&["--upload-everything"], None, None);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("unknown argument"));
}
