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
        "set_paused",
        "list_apps",
        "set_dry_run",
        "set_mapping",
        "remove_mapping",
        "apply_template",
        "fix_accessibility",
        "set_menu_item",
        "remove_menu_item",
        "move_menu_item",
        "list_displays",
        "start_fresh",
        "set_debug",
        "get_debug_log",
        "export_logs",
    ];
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build");
}
