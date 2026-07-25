//! Dossier surveillé : un fichier déposé déclenche une conversion automatique
//! vers un format préconfiguré (mêmes commandes que ConvertPanel/convert.rs,
//! réutilisées telles quelles — mêmes events `job-progress`/`job-done`).
//!
//! La sortie va toujours dans <dossier>/Converti, jamais surveillé (le watcher
//! n'est pas récursif) : ça évite tout risque de boucle de retraitement.

use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::convert::{start_convert_job, AUDIO_OR_VIDEO_EXT};
use crate::convert_format::{CONVERT_AUDIO_FORMATS, CONVERT_VIDEO_CONTAINERS};
use crate::jobs::JobRegistry;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WatchConfig {
    pub enabled: bool,
    pub folder: String,
    pub kind: String, // "convert-video" | "audio"
    pub target: String,
    pub loudnorm: bool,
}

#[derive(Default)]
pub struct WatchState {
    stop_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

fn config_path(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    std::fs::create_dir_all(&dir).ok();
    dir.join("watch_folder.json")
}

fn save_config(app: &AppHandle, cfg: &WatchConfig) {
    if let Ok(json) = serde_json::to_string(cfg) {
        std::fs::write(config_path(app), json).ok();
    }
}

#[tauri::command]
pub fn get_watch_config(app: AppHandle) -> WatchConfig {
    std::fs::read_to_string(config_path(&app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn stop_current(state: &WatchState) {
    if let Some(tx) = state.stop_tx.lock().unwrap().take() {
        let _ = tx.send(());
    }
}

#[tauri::command]
pub fn set_watch_config(
    app: AppHandle,
    state: tauri::State<'_, WatchState>,
    cfg: WatchConfig,
) -> Result<(), String> {
    stop_current(&state);
    if cfg.enabled {
        let target_ok = match cfg.kind.as_str() {
            "convert-video" => CONVERT_VIDEO_CONTAINERS.contains(&cfg.target.as_str()),
            "audio" => CONVERT_AUDIO_FORMATS.contains(&cfg.target.as_str()),
            _ => return Err("type de traitement invalide".into()),
        };
        if !target_ok {
            return Err("format cible invalide".into());
        }
        if !Path::new(&cfg.folder).is_dir() {
            return Err("dossier introuvable".into());
        }
    }
    save_config(&app, &cfg);
    if cfg.enabled {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *state.stop_tx.lock().unwrap() = Some(tx);
        spawn_watcher(app, cfg, rx);
    }
    Ok(())
}

/// Démarre le watcher persisté au lancement de l'app, s'il était actif.
pub fn start_if_enabled(app: &AppHandle, state: &WatchState) {
    let cfg = get_watch_config(app.clone());
    if cfg.enabled && Path::new(&cfg.folder).is_dir() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *state.stop_tx.lock().unwrap() = Some(tx);
        spawn_watcher(app.clone(), cfg, rx);
    }
}

fn spawn_watcher(app: AppHandle, cfg: WatchConfig, mut stop_rx: tokio::sync::oneshot::Receiver<()>) {
    let folder = PathBuf::from(&cfg.folder);
    let dest = folder.join("Converti");
    if std::fs::create_dir_all(&dest).is_err() {
        return;
    }

    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
    let mut watcher = match notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[watch-folder] init impossible : {e}");
            return;
        }
    };
    if let Err(e) = watcher.watch(&folder, RecursiveMode::NonRecursive) {
        eprintln!("[watch-folder] surveillance impossible : {e}");
        return;
    }

    // le thread bloquant garde `watcher` vivant jusqu'à l'arrêt demandé —
    // tauri::async_runtime::spawn_blocking (pas tokio::task::spawn_blocking) :
    // appelé aussi depuis .setup(), un contexte synchrone sans handle Tokio
    // ambiant, où spawn_blocking brut de tokio panique ("no reactor running")
    tauri::async_runtime::spawn_blocking(move || {
        let mut seen: HashSet<PathBuf> = HashSet::new();
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(event)) => {
                    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        continue;
                    }
                    for path in event.paths {
                        if seen.contains(&path) || !path.is_file() {
                            continue;
                        }
                        if path.parent() != Some(folder.as_path()) {
                            continue; // ignore Converti/ et sous-dossiers
                        }
                        let ext_ok = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| AUDIO_OR_VIDEO_EXT.contains(&e.to_lowercase().as_str()))
                            .unwrap_or(false);
                        if !ext_ok {
                            continue;
                        }
                        seen.insert(path.clone());
                        tauri::async_runtime::spawn(process_dropped_file(
                            app.clone(),
                            dest.clone(),
                            cfg.clone(),
                            path,
                        ));
                    }
                }
                Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(watcher);
    });
}

/// Attend la fin d'écriture (taille stable) avant de lancer la conversion —
/// un dépôt de fichier volumineux prend du temps à se copier sur le disque.
async fn process_dropped_file(app: AppHandle, dest: PathBuf, cfg: WatchConfig, path: PathBuf) {
    let mut last_size: Option<u64> = None;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let size = std::fs::metadata(&path).ok().map(|m| m.len());
        if size.is_some() && size == last_size {
            break;
        }
        last_size = size;
    }
    if !path.is_file() {
        return; // supprimé/déplacé entre-temps
    }
    let id = Uuid::new_v4().to_string();
    let registry = app.state::<JobRegistry>();
    let _ = start_convert_job(
        app.clone(),
        registry,
        id,
        cfg.kind.clone(),
        path.to_string_lossy().into_owned(),
        None,
        cfg.target.clone(),
        dest.to_string_lossy().into_owned(),
        cfg.loudnorm,
    )
    .await;
}
