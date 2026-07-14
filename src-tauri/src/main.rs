// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod format;
mod jobs;
mod tools;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(jobs::JobRegistry::default())
        .invoke_handler(tauri::generate_handler![
            tools::tools_status,
            tools::setup_tools,
            jobs::inspect_url,
            jobs::start_job,
            jobs::cancel_job,
            jobs::ai_rename,
            jobs::rename_file,
            jobs::default_download_dir
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de ForgeScoop");
}
