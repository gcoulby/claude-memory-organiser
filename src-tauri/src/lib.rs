mod commands;
mod error;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::settings::app_info])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        // Nothing else can report this: the window never opened.
        eprintln!("failed to start the application: {err}");
        std::process::exit(1);
    }
}
