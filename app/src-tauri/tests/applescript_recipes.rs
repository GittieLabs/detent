//! Every AppleScript on the docs site's recipes page compiles (D-126), so a
//! copied recipe never fails on a typo or a command the app doesn't have.
//! `osacompile` checks syntax and each app's dictionary without running
//! anything. A recipe whose app isn't installed here (Chrome or Zoom on a CI
//! runner, say) is skipped, and the test says which.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;

fn page() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../site/src/applescript.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The ```applescript blocks, in order.
fn scripts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```applescript") => current = Some(String::new()),
            (Some(s), "```") => {
                out.push(std::mem::take(s));
                current = None;
            }
            (Some(s), l) => {
                s.push_str(l);
                s.push('\n');
            }
            (None, _) => {}
        }
    }
    out
}

/// The apps a script tells, by name.
fn apps(script: &str) -> Vec<String> {
    script
        .split("application \"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next().map(str::to_owned))
        .collect()
}

/// Whether Launch Services knows an app by this name, without opening it.
fn installed(app: &str) -> bool {
    Command::new("/usr/bin/open")
        .args(["-Ra", app])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn the_page_has_recipes() {
    assert!(scripts(&page()).len() >= 10);
}

#[test]
fn every_recipe_compiles() {
    let dir = std::env::temp_dir().join(format!("detent-recipes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut skipped = Vec::new();
    for (i, script) in scripts(&page()).iter().enumerate() {
        let missing: Vec<String> = apps(script).into_iter().filter(|a| !installed(a)).collect();
        if !missing.is_empty() {
            skipped.push(format!("recipe {} (needs {})", i + 1, missing.join(", ")));
            continue;
        }
        let src = dir.join(format!("{i}.applescript"));
        std::fs::write(&src, script).unwrap();
        let out = Command::new("/usr/bin/osacompile")
            .arg("-o")
            .arg(dir.join(format!("{i}.scpt")))
            .arg(&src)
            .output()
            .expect("osacompile runs");
        assert!(
            out.status.success(),
            "recipe {} doesn't compile:\n{script}\n{}",
            i + 1,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    if !skipped.is_empty() {
        eprintln!("skipped: {}", skipped.join("; "));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
