use tauri::State;

use crate::db::queries::{
    self, ChatState, ConversationDetail, ConversationRow, LibraryStats, StateCounts,
};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn library_stats(state: State<'_, AppState>) -> AppResult<LibraryStats> {
    state.with_conn(queries::library_stats)
}

#[tauri::command]
pub fn list_conversations(
    state: State<'_, AppState>,
    filter: Option<ChatState>,
) -> AppResult<Vec<ConversationRow>> {
    state.with_conn(|c| queries::list_conversations(c, filter))
}

#[tauri::command]
pub fn state_counts(state: State<'_, AppState>) -> AppResult<StateCounts> {
    state.with_conn(queries::state_counts)
}

#[tauri::command]
pub fn get_conversation(
    state: State<'_, AppState>,
    id: i64,
    via: Option<String>,
) -> AppResult<ConversationDetail> {
    state.with_conn(|c| queries::get_conversation(c, id, via.as_deref()))
}

#[tauri::command]
pub fn set_conversation_state(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    new_state: ChatState,
) -> AppResult<()> {
    state.with_conn_mut(|c| queries::set_state(c, &ids, new_state))
}
