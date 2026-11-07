use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::import::{self, ImportProgress};
use crate::state::AppState;

pub const EVENT_PROGRESS: &str = "import://progress";
pub const EVENT_FINISHED: &str = "import://finished";
pub const EVENT_FAILED: &str = "import://failed";

/// Clears the running flag however the import ends.
struct RunningGuard<'a>(&'a AppState);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.import_running.store(false, Ordering::SeqCst);
    }
}

/// Asks for export files, then imports them in the background. Returns
/// false if the user closed the picker. Progress and the result arrive as
/// events, so the UI never waits on this call.
#[tauri::command]
pub async fn import_choose_and_start(app: AppHandle) -> AppResult<bool> {
    let state = app.state::<AppState>();
    if state.session().is_none() {
        return Err(AppError::Locked);
    }
    if state.job_running() {
        return Err(AppError::Busy);
    }

    let picked = app
        .dialog()
        .file()
        .set_title("Choose your Claude export (all the zips, or conversations.json)")
        .add_filter("Claude export", &["zip", "json"])
        .blocking_pick_files();
    let Some(picked) = picked else {
        return Ok(false);
    };
    let paths: Vec<PathBuf> = picked
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .collect();

    start(app.clone(), paths)?;
    Ok(true)
}

fn start(app: AppHandle, paths: Vec<PathBuf>) -> AppResult<()> {
    let state = app.state::<AppState>();
    if state.tagging_running.load(Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    if state
        .import_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(AppError::Busy);
    }
    state.import_cancel.store(false, Ordering::SeqCst);

    let worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker.state::<AppState>();
        let _running = RunningGuard(&state);
        match run(&worker, &state, &paths) {
            Ok(summary) => {
                let _ = worker.emit(EVENT_FINISHED, summary);
            }
            Err(err) => {
                // AppError is not Clone, which emit needs; its serialised form is.
                let _ = worker.emit(EVENT_FAILED, serde_json::to_value(&err).ok());
            }
        }
    });
    Ok(())
}

fn run(app: &AppHandle, state: &AppState, paths: &[PathBuf]) -> AppResult<import::ImportSummary> {
    // A separate connection, so reads elsewhere are not blocked while the
    // import holds its write transaction.
    let key = state
        .session()
        .as_ref()
        .map(|s| s.key.clone())
        .ok_or(AppError::Locked)?;
    let mut conn = db::open(&state.paths.db_file(), &key, &state.paths.backups_dir())?;
    import::run_import(
        &mut conn,
        paths,
        &state.import_cancel,
        &mut |p: &ImportProgress| {
            let _ = app.emit(EVENT_PROGRESS, p);
        },
    )
}

#[tauri::command]
pub fn import_cancel(state: State<'_, AppState>) {
    state.import_cancel.store(true, Ordering::SeqCst);
}
