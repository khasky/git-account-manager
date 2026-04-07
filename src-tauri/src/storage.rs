use crate::models::AppState;
use std::fs;
use std::path::PathBuf;

fn storage_dir() -> PathBuf {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    data_dir.join("git-account-manager")
}

fn storage_path() -> PathBuf {
    let dir = storage_dir();
    fs::create_dir_all(&dir).ok();
    dir.join("profiles.json")
}

pub fn load_state() -> AppState {
    let path = storage_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppState::default(),
    }
}

pub fn save_state(state: &AppState) -> Result<(), String> {
    let path = storage_path();
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}
