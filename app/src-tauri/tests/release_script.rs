//! The checks in `scripts/release.sh` (D-114, D-120), run against throwaway
//! git repos through `DETENT_RELEASE_ROOT`. Each stops before any build, tag
//! push or upload, so they're fast and touch nothing real. The first real
//! release is Keith's (acceptance.md M9).

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/release.sh")
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repo shaped like Detent's, versions agreeing, committed on main.
fn repo(name: &str, ui_version: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("detent-release-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["app", "firmware/src", "protocol/src"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    let write = |p: &str, t: &str| std::fs::write(dir.join(p), t).unwrap();
    write(
        "Cargo.toml",
        "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        "app/package.json",
        &format!("{{\n  \"name\": \"detent\",\n  \"version\": \"{ui_version}\",\n  \"private\": true\n}}\n"),
    );
    write(
        "firmware/src/version.h",
        "#define DETENT_FW_MAJOR 0\n#define DETENT_FW_MINOR 3\n#define DETENT_FW_PATCH 0\n",
    );
    write(
        "protocol/src/lib.rs",
        "pub const PROTOCOL_VERSION: u8 = 3;\n",
    );
    write(".gitignore", "target/\n");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "init"]);
    dir
}

/// Gives `dir` an origin that has its main, as after a push.
fn pushed(dir: &Path) {
    let bare = dir.with_extension("origin.git");
    let _ = std::fs::remove_dir_all(&bare);
    git(dir, &["init", "-q", "--bare", bare.to_str().unwrap()]);
    git(dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git(dir, &["push", "-q", "origin", "main"]);
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(script())
        .args(args)
        .env("DETENT_RELEASE_ROOT", dir)
        .output()
        .expect("scripts/release.sh runs")
}

fn fails_with(out: &Output, text: &str) {
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "should fail: {err}");
    assert!(err.contains(text), "wanted {text:?} in: {err}");
}

#[test]
fn rejects_unknown_arguments() {
    let dir = repo("args", "0.1.0");
    fails_with(&run(&dir, &["--force"]), "unknown argument: --force");
}

#[test]
fn the_app_versions_must_agree() {
    let dir = repo("versions", "0.2.0");
    fails_with(
        &run(&dir, &[]),
        "Cargo.toml says 0.1.0 but app/package.json says 0.2.0",
    );
}

#[test]
fn only_main_is_released() {
    let dir = repo("branch", "0.1.0");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    fails_with(
        &run(&dir, &[]),
        "on branch feature; releases are tagged on main",
    );
}

#[test]
fn the_tree_must_be_clean() {
    let dir = repo("dirty", "0.1.0");
    std::fs::write(dir.join("stray.txt"), "x").unwrap();
    fails_with(&run(&dir, &[]), "the working tree isn't clean");
}

#[test]
fn main_must_be_pushed() {
    let dir = repo("unpushed", "0.1.0");
    fails_with(&run(&dir, &[]), "no origin/main; push main first");
    pushed(&dir);
    std::fs::write(dir.join("later.txt"), "x").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "later"]);
    fails_with(
        &run(&dir, &[]),
        "HEAD isn't on origin/main; push main first",
    );
}

#[test]
fn an_existing_tag_is_never_reused() {
    let dir = repo("tagged", "0.1.0");
    pushed(&dir);
    git(&dir, &["tag", "-a", "v0.1.0", "-m", "earlier"]);
    fails_with(&run(&dir, &[]), "tag v0.1.0 already exists");
}

#[test]
fn the_notarized_dmg_must_be_built_first() {
    let dir = repo("no-dmg", "0.1.0");
    pushed(&dir);
    fails_with(&run(&dir, &[]), "Detent-0.1.0.dmg; run just build-release");
}

#[test]
fn an_unstapled_dmg_is_refused() {
    let dir = repo("unstapled", "0.1.0");
    pushed(&dir);
    let dist = dir.join("target/release/dist");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("Detent-0.1.0.dmg"), "not a disk image").unwrap();
    fails_with(&run(&dir, &[]), "has no valid stapled ticket");
}

#[test]
fn publishing_needs_the_tag_from_prepare() {
    let dir = repo("publish", "0.1.0");
    pushed(&dir);
    fails_with(
        &run(&dir, &["--publish"]),
        "no tag v0.1.0 on HEAD; run just release first",
    );
}
