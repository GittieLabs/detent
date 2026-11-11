//! Tauri commands for the settings window. Thin: each runs a config edit from
//! `detent_lib::app_state` (tested there), saves the config atomically,
//! reloads the engine, and publishes the new state. Only the settings window
//! may call them (capability `default`, D-071).

use detent_lib::app_state::{self, AppState, EditError};
use detent_lib::store::{self, Config, Mapping, MenuItem, Scope, Trigger};
use tauri::State;

use crate::launch::Shared;

/// Every command answers with the state after it ran, or a message to show.
type Answer = Result<AppState, String>;

fn snapshot(shared: &Shared) -> AppState {
    shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Applies `edit` to the config, saves it, reloads the engine, and publishes.
/// A refusal is logged as well as shown, so the log explains a failed save.
fn edit(shared: &Shared, change: impl FnOnce(&Config) -> Result<Config, EditError>) -> Answer {
    let answer = apply_edit(shared, change);
    if let Err(e) = &answer {
        shared.say(&format!("settings edit refused: {e}"));
    }
    answer
}

fn apply_edit(shared: &Shared, edit: impl FnOnce(&Config) -> Result<Config, EditError>) -> Answer {
    if !shared.writable {
        return Err(
            "The config file couldn't be read, so Detent won't overwrite it. \
                    Fix or remove it, then relaunch Detent."
                .into(),
        );
    }
    let path = shared.path.as_ref().ok_or("no place to save the config")?;
    let mut config = shared.config.lock().unwrap_or_else(|e| e.into_inner());
    let new = edit(&config).map_err(|e| e.to_string())?;
    store::save(path, &new).map_err(|e| e.to_string())?;
    if let Some(engine) = &shared.engine {
        engine.reload(new.clone());
    }
    let state = {
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.set_config(&new);
        state.clone()
    };
    *config = new;
    shared.publish(&state);
    Ok(state)
}

pub fn set_identifying(shared: &Shared, on: bool) -> Answer {
    if let Some(engine) = &shared.engine {
        engine.identify(on);
    }
    let state = {
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.set_identifying(on);
        state.clone()
    };
    shared.publish(&state);
    Ok(state)
}

/// Turns Pause on or off and saves it, so it survives a relaunch.
pub fn set_paused_now(shared: &Shared, on: bool) -> Answer {
    edit(shared, |c| Ok(app_state::set_paused(c, on)))
}

#[tauri::command]
pub fn set_paused(shared: State<'_, Shared>, on: bool) -> Answer {
    set_paused_now(&shared, on)
}

/// Adds a mapping or replaces the one with the same control, trigger and
/// scope, checked against the unit's controls from its HELLO.
#[tauri::command]
pub fn set_mapping(shared: State<'_, Shared>, mapping: Mapping) -> Answer {
    let controls = shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .paired
        .iter()
        .find(|u| u.serial.eq_ignore_ascii_case(&mapping.device))
        .map(|u| u.controls.clone())
        .unwrap_or_default();
    edit(&shared, |c| app_state::set_mapping(c, &controls, mapping))
}

#[tauri::command]
pub fn remove_mapping(
    shared: State<'_, Shared>,
    serial: String,
    control: u8,
    trigger: Trigger,
    scope: Scope,
) -> Answer {
    edit(&shared, |c| {
        app_state::remove_mapping(c, &serial, control, trigger, &scope)
    })
}

/// Adds a device menu item (`index` None) or replaces the one at `index`.
#[tauri::command]
pub fn set_menu_item(
    shared: State<'_, Shared>,
    serial: String,
    index: Option<usize>,
    item: MenuItem,
) -> Answer {
    edit(&shared, |c| {
        app_state::set_menu_item(c, &serial, index, item)
    })
}

#[tauri::command]
pub fn remove_menu_item(shared: State<'_, Shared>, serial: String, index: usize) -> Answer {
    edit(&shared, |c| app_state::remove_menu_item(c, &serial, index))
}

#[tauri::command]
pub fn move_menu_item(shared: State<'_, Shared>, serial: String, from: usize, to: usize) -> Answer {
    edit(&shared, |c| app_state::move_menu_item(c, &serial, from, to))
}

/// Adds the unit's starter template (D-077), keeping its own mappings.
#[tauri::command]
pub fn apply_template(shared: State<'_, Shared>, serial: String) -> Answer {
    edit(&shared, |c| app_state::apply_template(c, &serial))
}

/// The banner's Fix: macOS's Accessibility prompt and the Settings pane.
/// The state follows when the grant is next checked (within about 2 s).
#[tauri::command]
pub fn fix_accessibility(shared: State<'_, Shared>) -> AppState {
    #[cfg(target_os = "macos")]
    let granted = detent_lib::macos::request_accessibility();
    #[cfg(not(target_os = "macos"))]
    let granted = false;
    let state = {
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.set_accessibility(granted);
        state.clone()
    };
    shared.publish(&state);
    state
}

/// Turns dry run on or off (the Test screen, D-079). The state follows when
/// the engine confirms it, within about 20 ms.
pub fn set_dry_run_now(shared: &Shared, on: bool) {
    if let Some(engine) = &shared.engine {
        engine.set_dry_run(on);
    }
}

#[tauri::command]
pub fn set_dry_run(shared: State<'_, Shared>, on: bool) {
    set_dry_run_now(&shared, on);
}

/// Names of the connected displays, main first, for Open dashboard's
/// display picker. Sync, so it runs on the main thread like other window
/// work.
#[tauri::command]
pub fn list_displays(app: tauri::AppHandle) -> Vec<String> {
    crate::dashboard::display_names(&app)
}

/// Apps for the app picker (D-080). Async, so the folder scan runs off the
/// main thread.
#[tauri::command]
pub async fn list_apps() -> Vec<detent_lib::apps::AppEntry> {
    detent_lib::apps::list()
}

#[tauri::command]
pub fn get_state(shared: State<'_, Shared>) -> AppState {
    snapshot(&shared)
}

/// The wizard opened: unpaired units are opened read only (D-066).
#[tauri::command]
pub fn start_pairing(shared: State<'_, Shared>) -> Answer {
    set_identifying(&shared, true)
}

/// The wizard closed: candidates are closed.
#[tauri::command]
pub fn end_pairing(shared: State<'_, Shared>) -> Answer {
    set_identifying(&shared, false)
}

#[tauri::command]
pub fn pair(shared: State<'_, Shared>, vid: u16, pid: u16, serial: String, name: String) -> Answer {
    edit(&shared, |c| app_state::pair(c, vid, pid, &serial, &name))
}

#[tauri::command]
pub fn rename(shared: State<'_, Shared>, serial: String, name: String) -> Answer {
    edit(&shared, |c| app_state::rename(c, &serial, &name))
}

/// How many mappings unpairing this unit would remove, for the confirmation.
#[tauri::command]
pub fn mappings_of(shared: State<'_, Shared>, serial: String) -> usize {
    let config = shared.config.lock().unwrap_or_else(|e| e.into_inner());
    app_state::mappings_of(&config, &serial)
}

#[tauri::command]
pub fn unpair(shared: State<'_, Shared>, serial: String) -> Answer {
    edit(&shared, |c| app_state::unpair(c, &serial).map(|(c, _)| c))
}

#[tauri::command]
pub fn ignore(shared: State<'_, Shared>, vid: u16, pid: u16, serial: String) -> Answer {
    edit(&shared, |c| app_state::ignore(c, vid, pid, &serial))
}

#[tauri::command]
pub fn unignore(shared: State<'_, Shared>, serial: String) -> Answer {
    edit(&shared, |c| app_state::unignore(c, &serial))
}

/// Every command, checked against `build.rs` and the capability by a test.
#[cfg(test)]
pub const ALL: &[&str] = &[
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
];
