// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod format;
mod jobs;
mod tools;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(jobs::JobRegistry::default())
        // fermeture de l'app : tuer les yt-dlp actifs, sinon des orphelins
        // continuent d'écrire dans les fichiers que la reprise rouvrira ([Errno 22])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                use tauri::Manager;
                jobs::kill_all(&window.app_handle().state::<jobs::JobRegistry>());
            }
        })
        .invoke_handler(tauri::generate_handler![
            tools::tools_status,
            tools::setup_tools,
            tools::update_ytdlp,
            jobs::inspect_url,
            jobs::start_job,
            jobs::cancel_job,
            jobs::ai_rename,
            jobs::rename_file,
            jobs::open_file,
            jobs::show_in_folder,
            jobs::default_download_dir
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de ForgeScoop");
}
