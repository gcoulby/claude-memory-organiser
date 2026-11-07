mod commands;
mod db;
mod error;
mod import;
mod state;

use tauri::Manager;

use db::vault::VaultPaths;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(AppState::new(VaultPaths::new(dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::app_info,
            commands::vault::vault_status,
            commands::vault::vault_create,
            commands::vault::vault_unlock,
            commands::vault::vault_lock,
            commands::import::import_choose_and_start,
            commands::import::import_cancel,
            commands::chats::library_stats,
        ])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        // Nothing else can report this: the window never opened.
        eprintln!("failed to start the application: {err}");
        std::process::exit(1);
    }
}
