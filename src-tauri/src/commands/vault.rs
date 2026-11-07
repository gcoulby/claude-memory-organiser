use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::db::{self, vault};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultStatus {
    /// First run: no vault yet.
    New,
    Locked,
    Unlocked,
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    if state.session().is_some() {
        VaultStatus::Unlocked
    } else if state.paths.exists() {
        VaultStatus::Locked
    } else {
        VaultStatus::New
    }
}

#[tauri::command]
pub async fn vault_create(app: AppHandle, password: String) -> AppResult<()> {
    // Argon2 takes around half a second, so keep it off the main thread.
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let paths = &state.paths;
        let key = vault::create(paths, &password, vault::KdfParams::DEFAULT)?;
        let conn = db::open(&paths.db_file(), &key, &paths.backups_dir())?;
        *state.session() = Some(Session { key, conn });
        Ok(())
    })
    .await
    .map_err(AppError::Tauri)?
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, password: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let paths = &state.paths;
        let key = vault::unlock(paths, &password)?;
        let conn = db::open(&paths.db_file(), &key, &paths.backups_dir())?;
        *state.session() = Some(Session { key, conn });
        Ok(())
    })
    .await
    .map_err(AppError::Tauri)?
}

#[tauri::command]
pub fn vault_lock(state: State<'_, AppState>) -> AppResult<()> {
    if state.import_running.load(Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    // Dropping the session closes the connection and wipes the key.
    *state.session() = None;
    Ok(())
}
