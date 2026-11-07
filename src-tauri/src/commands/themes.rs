use tauri::State;

use crate::db::organise::{self, SegmentInfo, TaggingStats, ThemeRow, ThemeSegment};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn list_themes(state: State<'_, AppState>) -> AppResult<Vec<ThemeRow>> {
    state.with_conn(organise::list_themes)
}

#[tauri::command]
pub fn theme_segments(state: State<'_, AppState>, theme_id: i64) -> AppResult<Vec<ThemeSegment>> {
    state.with_conn(|c| organise::theme_segments(c, theme_id))
}

#[tauri::command]
pub fn conversation_segments(
    state: State<'_, AppState>,
    conversation_id: i64,
) -> AppResult<Vec<SegmentInfo>> {
    state.with_conn(|c| organise::conversation_segments(c, conversation_id))
}

#[tauri::command]
pub fn tagging_stats(state: State<'_, AppState>) -> AppResult<TaggingStats> {
    state.with_conn(organise::tagging_stats)
}
