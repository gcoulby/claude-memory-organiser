//! Tagging runs. Progress and results arrive as events; the UI never waits.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::settings::{AiMode, load_preferences};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::pipeline::{self, Estimate, Scope, TagProgress, llm::Anthropic};
use crate::state::AppState;

pub const EVENT_PROGRESS: &str = "tagging://progress";
pub const EVENT_FINISHED: &str = "tagging://finished";
pub const EVENT_FAILED: &str = "tagging://failed";

#[tauri::command]
pub fn tagging_estimate(state: State<'_, AppState>, scope: Scope) -> AppResult<Estimate> {
    state.with_conn(|conn| {
        let ids = pipeline::select(conn, scope)?;
        pipeline::estimate(conn, &ids)
    })
}

struct RunningGuard<'a>(&'a AppState);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.tagging_running.store(false, Ordering::SeqCst);
    }
}

/// Starts a tagging run in the background. Only in API mode, with a key.
#[tauri::command]
pub fn tagging_start(app: AppHandle, scope: Scope) -> AppResult<()> {
    let state = app.state::<AppState>();
    if load_preferences(&state).ai_mode != AiMode::Api {
        return Err(AppError::InvalidInput(
            "Tagging sends chat text to the Anthropic API. Turn on API mode in Settings first."
                .into(),
        ));
    }
    let key = crate::secrets::api_key()?.ok_or(AppError::NoApiKey)?;
    let db_key = state
        .session()
        .as_ref()
        .map(|s| s.key.clone())
        .ok_or(AppError::Locked)?;
    if state.import_running.load(Ordering::SeqCst)
        || state
            .tagging_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
    {
        return Err(AppError::Busy);
    }
    state.tagging_cancel.store(false, Ordering::SeqCst);

    let worker = app.clone();
    std::thread::spawn(move || {
        let state = worker.state::<AppState>();
        let _running = RunningGuard(&state);
        let result = (|| {
            let llm = Anthropic::new(key)?;
            // Its own connection, so reading the app stays responsive.
            let mut conn = db::open(&state.paths.db_file(), &db_key, &state.paths.backups_dir())?;
            let ids = pipeline::select(&conn, scope)?;
            pipeline::run(
                &mut conn,
                &llm,
                &ids,
                &state.tagging_cancel,
                &mut |p: &TagProgress| {
                    let _ = worker.emit(EVENT_PROGRESS, p);
                },
            )
        })();
        match result {
            Ok(summary) => {
                let _ = worker.emit(EVENT_FINISHED, summary);
            }
            Err(err) => {
                let _ = worker.emit(EVENT_FAILED, serde_json::to_value(&err).ok());
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn tagging_cancel(state: State<'_, AppState>) {
    state.tagging_cancel.store(true, Ordering::SeqCst);
}
