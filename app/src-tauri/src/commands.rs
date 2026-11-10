//! Tauri commands for the settings window. Thin: each runs a config edit from
//! `detent_lib::app_state` (tested there), saves the config atomically,
//! reloads the engine, and publishes the new state. Only the settings window
//! may call them (capability `default`, D-071).

use detent_lib::app_state::{self, AppState, EditError};
use detent_lib::store::{self, Config};
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
fn edit(shared: &Shared, edit: impl FnOnce(&Config) -> Result<Config, EditError>) -> Answer {
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
];
