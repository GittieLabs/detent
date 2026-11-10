fn main() {
    // The app's commands, declared so each gets an `allow-` permission and
    // only windows whose capability lists it may call it (D-071). Keep in
    // step with `src/commands.rs` (`ALL`); a test checks.
    const COMMANDS: &[&str] = &[
        "get_state",
        "start_pairing",
        "end_pairing",
        "pair",
        "rename",
        "mappings_of",
        "unpair",
        "ignore",
        "unignore",
    ];
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build");
}
