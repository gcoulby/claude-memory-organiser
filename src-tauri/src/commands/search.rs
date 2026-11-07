use tauri::State;

use crate::db::search::{self, SearchResults};
use crate::error::AppResult;
use crate::state::AppState;

const MAX_HITS: i64 = 200;

#[tauri::command]
pub fn search_messages(state: State<'_, AppState>, query: String) -> AppResult<SearchResults> {
    state.with_conn(|c| search::search(c, &query, MAX_HITS))
}
