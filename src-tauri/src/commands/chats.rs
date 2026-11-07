use tauri::State;

use crate::db::queries::{self, LibraryStats};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn library_stats(state: State<'_, AppState>) -> AppResult<LibraryStats> {
    state.with_conn(queries::library_stats)
}
