use tauri::{AppHandle, State, command};

use crate::*;

#[command]
pub fn remove_project(name: String, state: State<'_, AppState>, app: AppHandle) {
    let mut projects = state.projects.lock().unwrap();
    projects.remove(&name);

    if let Err(err) = update(&app, &projects) {
        eprintln!("Error sending update: {}", err);
    }
}

#[command]
pub fn get_key(key: String, state: State<'_, AppState>) -> Result<String, ()> {
    state.options.get_raw_key(&key).ok_or(())
}

#[command]
pub fn set_key(key: String, value: String, state: State<'_, AppState>) {
    println!("Setting key {} to {}", key, value);
    state.options.set_raw_key(key, value);
}