mod cleaner;
mod commands;
mod disk;
mod error;
mod guard;
mod history;
mod rules;
mod scanner;
mod state;

use std::sync::Arc;

pub use cleaner::admin::helper_main;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(state::AppState::default()))
        .invoke_handler(tauri::generate_handler![
            disk::volumes_list,
            commands::scan_start,
            commands::scan_cancel,
            commands::level_get,
            commands::children_get,
            commands::reveal_path,
            commands::open_fda_settings,
            commands::delete_run,
            commands::history_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running DiskViet");
}
