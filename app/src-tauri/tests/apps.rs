//! The app picker's list: installed bundles read from their Info.plist, merged
//! with the running apps by bundle ID (D-080).

use std::fs;
use std::path::{Path, PathBuf};

use detent_lib::apps::{self, AppEntry};

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("detent-apps-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Writes `dir/<file>.app/Contents/Info.plist` from key and value pairs.
fn bundle(dir: &Path, file: &str, keys: &[(&str, &str)]) -> PathBuf {
    let app = dir.join(format!("{file}.app"));
    fs::create_dir_all(app.join("Contents")).unwrap();
    let mut dict = plist::Dictionary::new();
    for (k, v) in keys {
        dict.insert((*k).into(), plist::Value::String((*v).into()));
    }
    plist::Value::Dictionary(dict)
        .to_file_xml(app.join("Contents/Info.plist"))
        .unwrap();
    app
}

fn entry(bundle_id: &str, name: &str, running: bool) -> AppEntry {
    AppEntry {
        bundle_id: bundle_id.into(),
        name: name.into(),
        running,
    }
}

#[test]
fn a_bundle_is_named_as_finder_shows_it() {
    let d = temp_dir("names");
    // Finder shows the bundle's file name, not CFBundleName ("Code").
    let app = bundle(
        &d,
        "Visual Studio Code",
        &[
            ("CFBundleIdentifier", "com.microsoft.VSCode"),
            ("CFBundleName", "Code"),
        ],
    );
    assert_eq!(
        apps::read_bundle(&app),
        Some(entry("com.microsoft.VSCode", "Visual Studio Code", false))
    );
}

#[test]
fn a_binary_plist_reads_too() {
    let d = temp_dir("binary");
    let app = d.join("Mail.app");
    fs::create_dir_all(app.join("Contents")).unwrap();
    let mut dict = plist::Dictionary::new();
    dict.insert("CFBundleIdentifier".into(), "com.apple.mail".into());
    plist::Value::Dictionary(dict)
        .to_file_binary(app.join("Contents/Info.plist"))
        .unwrap();
    assert_eq!(
        apps::read_bundle(&app),
        Some(entry("com.apple.mail", "Mail", false))
    );
}

#[test]
fn bundles_without_an_id_or_plist_are_skipped() {
    let d = temp_dir("skipped");
    let no_id = bundle(&d, "Helper", &[("CFBundleName", "Helper")]);
    let no_plist = d.join("Broken.app");
    fs::create_dir_all(&no_plist).unwrap();
    let empty_id = bundle(&d, "Empty", &[("CFBundleIdentifier", "  ")]);
    assert_eq!(apps::read_bundle(&no_id), None);
    assert_eq!(apps::read_bundle(&no_plist), None);
    assert_eq!(apps::read_bundle(&empty_id), None);
}

#[test]
fn a_folder_scan_finds_apps_one_level_down_and_ignores_the_rest() {
    let d = temp_dir("scan");
    bundle(&d, "Safari", &[("CFBundleIdentifier", "com.apple.Safari")]);
    bundle(&d, "Notes", &[("CFBundleIdentifier", "com.apple.Notes")]);
    fs::write(d.join("README.txt"), "not an app").unwrap();
    // Apps inside an app or a plain folder aren't offered.
    let nested = d.join("Tools");
    fs::create_dir_all(&nested).unwrap();
    bundle(
        &nested,
        "Deep",
        &[("CFBundleIdentifier", "com.example.deep")],
    );

    let mut found = apps::installed_in(&[d.clone(), d.join("missing")]);
    found.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        found,
        vec![
            entry("com.apple.Notes", "Notes", false),
            entry("com.apple.Safari", "Safari", false),
        ]
    );
}

#[test]
fn merging_marks_running_apps_dedupes_and_sorts_by_name() {
    let installed = vec![
        entry("com.apple.Safari", "Safari", false),
        entry("com.apple.mail", "Mail", false),
        entry("com.apple.Safari", "Safari copy", false),
        entry("com.apple.Notes", "notes", false),
    ];
    let running = vec![
        entry("com.apple.Safari", "Safari", true),
        // Running but not installed in a scanned folder (for example from a
        // disk image) still counts.
        entry("com.example.dmg", "From a disk", true),
    ];
    assert_eq!(
        apps::merge(running, installed),
        vec![
            entry("com.example.dmg", "From a disk", true),
            entry("com.apple.mail", "Mail", false),
            entry("com.apple.Notes", "notes", false),
            entry("com.apple.Safari", "Safari", true),
        ]
    );
}

#[test]
fn bundle_ids_match_ignoring_case_like_the_router() {
    let merged = apps::merge(
        vec![entry("com.apple.safari", "Safari", true)],
        vec![entry("com.apple.Safari", "Safari", false)],
    );
    assert_eq!(merged.len(), 1);
    assert!(merged[0].running);
}

#[cfg(target_os = "macos")]
#[test]
fn this_mac_lists_finder_and_safari() {
    let all = apps::list();
    let ids: Vec<&str> = all.iter().map(|a| a.bundle_id.as_str()).collect();
    assert!(ids.contains(&"com.apple.Safari"), "{ids:?}");
    // Finder lives in /System/Library/CoreServices, outside the scanned
    // folders, but it's always running in a GUI session.
    if std::env::var_os("CI").is_none() {
        assert!(ids.contains(&"com.apple.finder"), "{ids:?}");
    }
}
