// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard_watch;
mod convert;
mod convert_format;
mod cookies;
mod format;
mod jobs;
mod tools;
mod watch_folder;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(jobs::JobRegistry::default())
        .manage(watch_folder::WatchState::default())
        .manage(clipboard_watch::ClipboardState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            watch_folder::start_if_enabled(&handle, &handle.state::<watch_folder::WatchState>());

            let show = MenuItem::with_id(app, "show", "Afficher ForgeScoop", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            // pas d'icône embarquée (config bundle absente/échec de chargement) :
            // dégrader sans icône de zone de notification plutôt que planter l'app
            let Some(icon) = app.default_window_icon().cloned() else {
                eprintln!("[tray] icône par défaut indisponible, zone de notification désactivée");
                return Ok(());
            };
            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        jobs::kill_all(&app.state::<jobs::JobRegistry>());
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        // fermeture de la fenêtre : réduite dans la zone de notification plutôt
        // que fermée — les téléchargements/conversions/le dossier surveillé
        // continuent en tâche de fond ; "Quitter" dans le menu de l'icône
        // système tue les yt-dlp actifs et ferme réellement l'application
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.hide().ok();
            }
        })
        .invoke_handler(tauri::generate_handler![
            tools::tools_status,
            tools::setup_tools,
            tools::update_ytdlp,
            jobs::inspect_url,
            jobs::start_job,
            jobs::cancel_job,
            convert::start_convert_job,
            convert::probe_info,
            convert::list_media_files,
            jobs::ai_rename,
            jobs::rename_file,
            jobs::open_file,
            jobs::show_in_folder,
            jobs::default_download_dir,
            jobs::read_text_list,
            watch_folder::get_watch_config,
            watch_folder::set_watch_config,
            clipboard_watch::start_clipboard_watch,
            clipboard_watch::stop_clipboard_watch,
            cookies::cookies_status,
            cookies::set_cookies_file,
            cookies::clear_cookies_file
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de ForgeScoop");
}
