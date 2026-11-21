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
        // Must be registered first: a second launch only focuses the running window, so two
        // instances never race on the same bucket.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
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
