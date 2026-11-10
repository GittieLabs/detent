//! The app picker's list (D-080): apps installed in the usual folders, read
//! from each bundle's `Info.plist`, merged with the running apps by bundle ID.
//! Pure except for [`list`], which reads this Mac.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// One app the user can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppEntry {
    pub bundle_id: String,
    /// As Finder or the Dock shows it.
    pub name: String,
    pub running: bool,
}

/// Reads an `.app` bundle: its ID from `Contents/Info.plist` (XML or
/// binary), its name from the file name, as Finder shows it. `None` when
/// there's no plist or no bundle ID.
pub fn read_bundle(app: &Path) -> Option<AppEntry> {
    let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
    let id = info
        .as_dictionary()?
        .get("CFBundleIdentifier")?
        .as_string()?
        .trim();
    if id.is_empty() {
        return None;
    }
    let name = app.file_stem()?.to_str()?.to_owned();
    Some(AppEntry {
        bundle_id: id.to_owned(),
        name,
        running: false,
    })
}

/// The `.app` bundles directly inside each folder. Missing folders, other
/// files, and apps nested deeper are skipped.
pub fn installed_in(folders: &[PathBuf]) -> Vec<AppEntry> {
    let mut found = Vec::new();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "app") {
                found.extend(read_bundle(&path));
            }
        }
    }
    found
}

/// One entry per bundle ID (ignoring case, as the router does): running apps
/// first, so their names and `running` win, then installed ones. Sorted by
/// name, ignoring case.
pub fn merge(running: Vec<AppEntry>, installed: Vec<AppEntry>) -> Vec<AppEntry> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<AppEntry> = Vec::new();
    for app in running.into_iter().chain(installed) {
        if seen.insert(app.bundle_id.to_lowercase()) {
            out.push(app);
        }
    }
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.bundle_id.cmp(&b.bundle_id))
    });
    out
}

/// The folders scanned for installed apps.
pub fn app_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = [
        "/Applications",
        "/Applications/Utilities",
        "/System/Applications",
        "/System/Applications/Utilities",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = std::env::var_os("HOME") {
        folders.push(PathBuf::from(home).join("Applications"));
    }
    folders
}

/// Every app on this Mac the picker offers: running regular apps plus the
/// ones installed in [`app_folders`].
pub fn list() -> Vec<AppEntry> {
    #[cfg(target_os = "macos")]
    let running = crate::macos::running_apps()
        .into_iter()
        .map(|(bundle_id, name)| AppEntry {
            bundle_id,
            name,
            running: true,
        })
        .collect();
    #[cfg(not(target_os = "macos"))]
    let running = Vec::new();
    merge(running, installed_in(&app_folders()))
}
