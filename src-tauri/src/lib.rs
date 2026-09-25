pub mod catalog;
mod commands;
pub mod exec;
pub mod guards;
pub mod measure;
pub mod names;
pub mod paths;
pub mod platform;
pub mod process;
pub mod projects;
pub mod safety;
pub mod scan;
pub mod size;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = commands::AppState::new().expect("the embedded catalog must be valid");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(|app| {
            tray::create(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::cancel_scan,
            commands::simulate,
            commands::execute,
            commands::reveal,
            commands::open_full_disk_access_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
