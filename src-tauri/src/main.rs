// Stops an extra console window opening on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    memory_organiser_lib::run()
}
