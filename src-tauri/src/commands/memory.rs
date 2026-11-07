use tauri::State;

use crate::db::organise::{self, MemoryRow, MemoryStatus};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn list_memory(state: State<'_, AppState>) -> AppResult<Vec<MemoryRow>> {
    state.with_conn(organise::list_memory)
}

#[tauri::command]
pub fn set_memory_status(
    state: State<'_, AppState>,
    id: i64,
    status: MemoryStatus,
) -> AppResult<()> {
    state.with_conn(|c| organise::set_memory_status(c, id, status))
}
