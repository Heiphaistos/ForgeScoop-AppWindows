//! Installation automatique de yt-dlp et ffmpeg au premier lancement.
//! Les binaires vivent dans %APPDATA%/org.heiphaistos.forgescoop/bin.

use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const FFMPEG_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";

#[derive(Serialize, Clone)]
pub struct ToolsStatus {
    pub ytdlp: bool,
    pub ffmpeg: bool,
}

pub fn bin_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("app_data_dir indisponible")
        .join("bin");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn ytdlp_path(app: &AppHandle) -> PathBuf {
    bin_dir(app).join("yt-dlp.exe")
}

pub fn ffmpeg_dir(app: &AppHandle) -> PathBuf {
    bin_dir(app)
}

#[tauri::command]
pub fn tools_status(app: AppHandle) -> ToolsStatus {
    ToolsStatus {
        ytdlp: ytdlp_path(&app).exists(),
        ffmpeg: bin_dir(&app).join("ffmpeg.exe").exists(),
    }
}

fn emit_setup(app: &AppHandle, step: &str, progress: f64) {
    app.emit("setup-progress", serde_json::json!({ "step": step, "progress": progress }))
        .ok();
}

async fn download_file(
    app: &AppHandle,
    url: &str,
    dest: &PathBuf,
    step: &str,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let res = reqwest::get(url).await.map_err(|e| format!("téléchargement {step} : {e}"))?;
    if !res.status().is_success() {
        return Err(format!("téléchargement {step} : HTTP {}", res.status()));
    }
    let total = res.content_length().unwrap_or(0);
    let tmp = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
    let mut stream = res.bytes_stream();
    let mut got: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        got += chunk.len() as u64;
        if total > 0 {
            emit_setup(app, step, got as f64 / total as f64 * 100.0);
        }
    }
    file.flush().await.ok();
    drop(file);
    tokio::fs::rename(&tmp, dest).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Télécharge yt-dlp.exe et ffmpeg.exe (extrait du zip essentials) si absents.
#[tauri::command]
pub async fn setup_tools(app: AppHandle) -> Result<ToolsStatus, String> {
    let bin = bin_dir(&app);

    let ytdlp = ytdlp_path(&app);
    if !ytdlp.exists() {
        emit_setup(&app, "yt-dlp", 0.0);
        download_file(&app, YTDLP_URL, &ytdlp, "yt-dlp").await?;
    }

    let ffmpeg = bin.join("ffmpeg.exe");
    if !ffmpeg.exists() {
        emit_setup(&app, "ffmpeg", 0.0);
        let zip_path = bin.join("ffmpeg.zip");
        download_file(&app, FFMPEG_URL, &zip_path, "ffmpeg").await?;
        emit_setup(&app, "extraction", 0.0);

        // extraction bloquante déportée
        let bin2 = bin.clone();
        let zip2 = zip_path.clone();
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            let file = std::fs::File::open(&zip2).map_err(|e| e.to_string())?;
            let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                let name = entry.name().replace('\\', "/");
                let base = name.rsplit('/').next().unwrap_or("");
                if base == "ffmpeg.exe" || base == "ffprobe.exe" {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                    std::fs::write(bin2.join(base), buf).map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())??;
        std::fs::remove_file(&zip_path).ok();
    }

    emit_setup(&app, "terminé", 100.0);
    Ok(tools_status(app))
}
