//! Conversion vidéo/audio, extraction audio, fusion audio+vidéo — via ffmpeg
//! en local (aucun serveur). Même mécanique que jobs.rs : PID partagé dans
//! JobRegistry (cancel_job/kill_all génériques), events Tauri `job-progress`/
//! `job-done` réutilisés tels quels côté frontend.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::convert_format::{
    audio_args, audio_for_container, video_args, CONVERT_AUDIO_FORMATS, CONVERT_VIDEO_CONTAINERS,
    MUX_CONTAINERS,
};
use crate::jobs::{register_pid, unregister_pid, DoneEvent, JobRegistry, ProgressEvent};
use crate::tools::{ffmpeg_path, ffprobe_path};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn unique_target(dest_dir: &PathBuf, name: &str) -> PathBuf {
    let mut target = dest_dir.join(name);
    let mut n = 1;
    while target.exists() {
        let stem = PathBuf::from(name);
        let ext = stem.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let base = stem.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_string());
        target = dest_dir.join(if ext.is_empty() { format!("{base} ({n})") } else { format!("{base} ({n}).{ext}") });
        n += 1;
    }
    target
}

fn base_name(path: &str) -> String {
    PathBuf::from(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "fichier".into())
}

async fn probe_duration(app: &AppHandle, input: &str) -> f64 {
    let mut cmd = Command::new(ffprobe_path(app));
    cmd.args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", input]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    match cmd.output().await {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

const VIDEO_EXT: [&str; 9] = ["mp4", "mkv", "webm", "mov", "avi", "wmv", "flv", "m4v", "ts"];
const AUDIO_OR_VIDEO_EXT: [&str; 16] = [
    "mp3", "m4a", "aac", "opus", "flac", "wav", "ogg",
    "mp4", "mkv", "webm", "mov", "avi", "wmv", "flv", "m4v", "ts",
];

/// Liste les fichiers médias d'un dossier (non récursif) — alimente le
/// sélecteur de contenu quand l'utilisateur choisit un dossier entier à convertir.
#[tauri::command]
pub fn list_media_files(dir: String, mode: String) -> Result<Vec<String>, String> {
    let exts: &[&str] = if mode == "video" { &VIDEO_EXT } else { &AUDIO_OR_VIDEO_EXT };
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("dossier illisible : {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    out.sort();
    Ok(out)
}

#[derive(Serialize)]
pub struct ProbeInfo {
    pub duration: f64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Aperçu léger avant conversion (durée + résolution) — pas de miniature visuelle
/// côté desktop (le dialogue Tauri ne donne qu'un chemin, pas les octets du fichier).
#[tauri::command]
pub async fn probe_info(app: AppHandle, path: String) -> Result<ProbeInfo, String> {
    let mut cmd = Command::new(ffprobe_path(&app));
    cmd.args([
        "-v", "error",
        "-show_entries", "format=duration:stream=width,height",
        "-of", "json",
        &path,
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().await.map_err(|e| format!("ffprobe : {e}"))?;
    if !out.status.success() {
        return Err("fichier illisible par ffprobe".into());
    }
    let data: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|_| "réponse ffprobe illisible".to_string())?;
    let duration = data["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let stream = data["streams"].as_array().and_then(|a| a.first());
    let width = stream.and_then(|s| s["width"].as_u64()).map(|n| n as u32);
    let height = stream.and_then(|s| s["height"].as_u64()).map(|n| n as u32);
    Ok(ProbeInfo { duration, width, height })
}

#[tauri::command]
pub async fn start_convert_job(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    id: String,
    kind: String,
    input: String,
    input2: Option<String>,
    target: String,
    dest: String,
    loudnorm: bool,
) -> Result<(), String> {
    let dest_dir = PathBuf::from(&dest);
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("dossier de destination : {e}"))?;

    let (args, out_path, duration_source): (Vec<String>, PathBuf, String) = match kind.as_str() {
        "convert-video" => {
            if !CONVERT_VIDEO_CONTAINERS.contains(&target.as_str()) {
                return Err("format vidéo invalide".into());
            }
            if !PathBuf::from(&input).is_file() {
                return Err("fichier source introuvable".into());
            }
            let out = unique_target(&dest_dir, &format!("{}.{}", base_name(&input), target));
            let mut a = vec!["-i".to_string(), input.clone()];
            a.extend(video_args(&target).ok_or("format vidéo invalide")?);
            a.push(out.to_string_lossy().into_owned());
            (a, out, input.clone())
        }
        "audio" => {
            if !CONVERT_AUDIO_FORMATS.contains(&target.as_str()) {
                return Err("format audio invalide".into());
            }
            if !PathBuf::from(&input).is_file() {
                return Err("fichier source introuvable".into());
            }
            let out = unique_target(&dest_dir, &format!("{}.{}", base_name(&input), target));
            let mut a = vec!["-i".to_string(), input.clone(), "-vn".to_string()];
            if loudnorm {
                a.push("-af".to_string());
                a.push("loudnorm".to_string());
            }
            a.extend(audio_args(&target).ok_or("format audio invalide")?);
            a.push(out.to_string_lossy().into_owned());
            (a, out, input.clone())
        }
        "mux" => {
            if !MUX_CONTAINERS.contains(&target.as_str()) {
                return Err("format de fusion invalide".into());
            }
            let audio_in = input2.clone().ok_or("fichier audio requis")?;
            if !PathBuf::from(&input).is_file() || !PathBuf::from(&audio_in).is_file() {
                return Err("fichier(s) source introuvable(s)".into());
            }
            let out = unique_target(&dest_dir, &format!("{}-fusion.{}", base_name(&input), target));
            let mut a = vec![
                "-i".to_string(), input.clone(),
                "-i".to_string(), audio_in,
                "-map".to_string(), "0:v:0".to_string(),
                "-map".to_string(), "1:a:0".to_string(),
                "-c:v".to_string(), "copy".to_string(),
            ];
            a.extend(audio_for_container(&target).ok_or("format de fusion invalide")?);
            a.push("-shortest".to_string());
            a.push(out.to_string_lossy().into_owned());
            (a, out, input.clone())
        }
        _ => return Err("type de traitement invalide".into()),
    };

    let duration = probe_duration(&app, &duration_source).await;

    let mut cmd = Command::new(ffmpeg_path(&app));
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-nostats"]);
    cmd.args(&args);
    cmd.args(["-progress", "pipe:1"]);
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("lancement ffmpeg : {e}"))?;

    if let Some(pid) = child.id() {
        register_pid(&registry, id.clone(), pid);
    }

    let stdout = child.stdout.take().ok_or("stdout indisponible")?;
    let stderr = child.stderr.take().ok_or("stderr indisponible")?;
    let app2 = app.clone();
    let id2 = id.clone();

    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buf: Vec<u8> = Vec::new();
        let mut last_pct = -1.0f64;
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim_end_matches(['\r', '\n']);
            if let Some(rest) = line.strip_prefix("out_time_us=") {
                if duration > 0.0 {
                    if let Ok(us) = rest.parse::<f64>() {
                        let pct = (us / 1_000_000.0 / duration * 100.0).min(100.0);
                        if pct - last_pct >= 0.5 {
                            last_pct = pct;
                            app2.emit("job-progress", ProgressEvent {
                                id: id2.clone(), progress: pct, speed: String::new(), eta: String::new(),
                                item_index: None, item_count: None, item_title: None,
                            }).ok();
                        }
                    }
                }
            }
        }
    });

    // stderr : garder la dernière ligne non vide comme message d'erreur
    let err_buf = Arc::new(Mutex::new(String::new()));
    let err_buf2 = err_buf.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf: Vec<u8> = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim_end_matches(['\r', '\n']);
            if !line.is_empty() {
                *err_buf2.lock().unwrap() = line.to_string();
            }
        }
    });

    let status = child.wait().await.map_err(|e| e.to_string())?;
    unregister_pid(&registry, &id);

    let (ok, error, files) = if status.success() {
        (true, None, vec![out_path.to_string_lossy().into_owned()])
    } else {
        let e = err_buf.lock().unwrap().clone();
        (false, Some(if e.is_empty() { "conversion annulée ou échouée".to_string() } else { e }), vec![])
    };
    app.emit("job-done", DoneEvent { id, ok, error, files }).ok();
    Ok(())
}
