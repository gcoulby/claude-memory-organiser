mod commands;
mod db;
mod error;
mod import;
mod pipeline;
mod secrets;
mod state;
mod tree;

use tauri::Manager;

use db::vault::VaultPaths;
use state::AppState;

/// Debug builds can point at another folder with `MO_DATA_DIR`, so the app
/// can be tried against a scratch vault without touching the real one.
/// Release builds always use the app data folder.
fn data_dir(app: &tauri::App) -> tauri::Result<std::path::PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("MO_DATA_DIR") {
        return Ok(std::path::PathBuf::from(dir));
    }
    app.path().app_data_dir()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let dir = data_dir(app)?;
            app.manage(AppState::new(VaultPaths::new(dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::app_info,
            commands::settings::get_preferences,
            commands::settings::set_preferences,
            commands::settings::backup_now,
            commands::settings::export_edits,
            commands::settings::api_key_present,
            commands::settings::set_api_key,
            commands::settings::clear_api_key,
            commands::jobs::tagging_estimate,
            commands::jobs::tagging_start,
            commands::jobs::tagging_cancel,
            commands::themes::list_themes,
            commands::themes::theme_segments,
            commands::themes::conversation_segments,
            commands::themes::tagging_stats,
            commands::memory::list_memory,
            commands::memory::set_memory_status,
            commands::chats::set_conversation_excluded,
            commands::vault::vault_status,
            commands::vault::vault_create,
            commands::vault::vault_unlock,
            commands::vault::vault_lock,
            commands::import::import_choose_and_start,
            commands::import::import_cancel,
            commands::chats::library_stats,
            commands::chats::list_conversations,
            commands::chats::state_counts,
            commands::chats::get_conversation,
            commands::chats::set_conversation_state,
            commands::search::search_messages,
        ])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        // Nothing else can report this: the window never opened.
        eprintln!("failed to start the application: {err}");
        std::process::exit(1);
    }
}
