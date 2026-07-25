//! Presse-papier surveillé : copier un lien reconnu affiche une suggestion de
//! téléchargement côté frontend (`clipboard-url` event), sans rien lancer
//! automatiquement — l'utilisateur choisit d'accepter ou d'ignorer.

use arboard::Clipboard;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

use crate::jobs::validate_url;

#[derive(Default)]
pub struct ClipboardState {
    stop_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

fn stop_current(state: &ClipboardState) {
    if let Some(tx) = state.stop_tx.lock().unwrap().take() {
        let _ = tx.send(());
    }
}

#[tauri::command]
pub fn start_clipboard_watch(app: AppHandle, state: State<'_, ClipboardState>) {
    stop_current(&state);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    *state.stop_tx.lock().unwrap() = Some(tx);

    tauri::async_runtime::spawn(async move {
        let mut last_text = String::new();
        let mut last_suggested = String::new();
        loop {
            tokio::time::sleep(Duration::from_millis(1200)).await;
            if rx.try_recv().is_ok() {
                break;
            }
            let text = match Clipboard::new().and_then(|mut c| c.get_text()) {
                Ok(t) => t,
                Err(_) => continue, // presse-papier vide/non-texte/verrouillé par une autre app
            };
            if text == last_text {
                continue;
            }
            last_text = text.clone();
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed == last_suggested || trimmed.len() > 2048 {
                continue;
            }
            if let Ok(url) = validate_url(trimmed) {
                last_suggested = trimmed.to_string();
                app.emit("clipboard-url", url).ok();
            }
        }
    });
}

#[tauri::command]
pub fn stop_clipboard_watch(state: State<'_, ClipboardState>) {
    stop_current(&state);
}
