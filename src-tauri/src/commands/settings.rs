use std::fs;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::db::{self, BackupKind, queries};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppResult<AppInfo> {
    let pkg = app.package_info();
    Ok(AppInfo {
        name: pkg.name.clone(),
        version: pkg.version.to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub check_updates_on_startup: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            check_updates_on_startup: true,
        }
    }
}

/// A missing or unreadable file means defaults: preferences are never
/// worth failing startup over.
#[tauri::command]
pub fn get_preferences(state: State<'_, AppState>) -> Preferences {
    fs::read(state.paths.preferences_file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn set_preferences(state: State<'_, AppState>, preferences: Preferences) -> AppResult<()> {
    let path = state.paths.preferences_file();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&preferences)?)?;
    Ok(())
}

/// Returns the backup's file name.
#[tauri::command]
pub fn backup_now(state: State<'_, AppState>) -> AppResult<String> {
    // The import writes through its own connection; copying mid-write
    // would capture a half-finished file.
    if state.import_running.load(Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    let paths = state.paths.clone();
    state.with_conn(|conn| {
        let dest = db::backup(
            conn,
            &paths.db_file(),
            &paths.backups_dir(),
            BackupKind::Manual,
            "manual",
        )?;
        Ok(dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default())
    })
}

/// Saves the user's own decisions as JSON. Returns false if the user
/// closed the save dialog.
#[tauri::command]
pub async fn export_edits(app: AppHandle) -> AppResult<bool> {
    let state = app.state::<AppState>();
    let edits = state.with_conn(queries::export_edits)?;
    let Some(dest) = app
        .dialog()
        .file()
        .set_title("Export my edits")
        .set_file_name("memory-organiser-edits.json")
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let dest = dest
        .into_path()
        .map_err(|_| AppError::InvalidInput("That location cannot be written to.".into()))?;
    fs::write(dest, serde_json::to_vec_pretty(&edits)?)?;
    Ok(true)
}
