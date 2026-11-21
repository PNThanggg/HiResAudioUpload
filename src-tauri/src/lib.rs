pub mod catalog;
pub mod commands;
pub mod error;
pub mod pipeline;
pub mod r2;
pub mod settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            let state = tauri::async_runtime::block_on(commands::init_state(path));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::test_connection,
            commands::list_tracks,
            commands::get_track,
            commands::upload_files,
            commands::update_track,
            commands::delete_tracks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
