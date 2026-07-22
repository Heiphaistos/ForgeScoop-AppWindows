//! Installation automatique de yt-dlp et ffmpeg au premier lancement.
//! Les binaires vivent dans %APPDATA%/org.heiphaistos.forgescoop/bin.

use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const FFMPEG_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
// Runtime JS exigé par yt-dlp pour YouTube (déchiffrement des signatures)
const DENO_URL: &str =
    "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";

#[derive(Serialize, Clone)]
pub struct ToolsStatus {
    pub ytdlp: bool,
    pub ffmpeg: bool,
    pub deno: bool,
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

pub fn ffmpeg_path(app: &AppHandle) -> PathBuf {
    bin_dir(app).join("ffmpeg.exe")
}

pub fn ffprobe_path(app: &AppHandle) -> PathBuf {
    bin_dir(app).join("ffprobe.exe")
}

pub fn deno_path(app: &AppHandle) -> PathBuf {
    bin_dir(app).join("deno.exe")
}

#[tauri::command]
pub fn tools_status(app: AppHandle) -> ToolsStatus {
    ToolsStatus {
        ytdlp: ytdlp_path(&app).exists(),
        ffmpeg: bin_dir(&app).join("ffmpeg.exe").exists(),
        deno: deno_path(&app).exists(),
    }
}

async fn extract_from_zip(
    zip_path: PathBuf,
    bin: PathBuf,
    wanted: &'static [&'static str],
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut found: Vec<&str> = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().replace('\\', "/");
            let base = name.rsplit('/').next().unwrap_or("").to_string();
            if let Some(&w) = wanted.iter().find(|&&w| w == base) {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                std::fs::write(bin.join(&base), buf).map_err(|e| e.to_string())?;
                found.push(w);
            }
        }
        // extraction silencieuse impossible : si l'archive amont change de
        // structure (fichier renommé/déplacé), signaler clairement plutôt que
        // de laisser tools_status()/ffmpeg_path() échouer sans explication plus tard
        let missing: Vec<&str> = wanted.iter().filter(|w| !found.contains(w)).copied().collect();
        if !missing.is_empty() {
            return Err(format!("archive inattendue : {} introuvable(s) dans le zip téléchargé", missing.join(", ")));
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
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

#[derive(Serialize)]
pub struct UpdateResult {
    pub updated: bool,
    pub version: String,
}

/// Met à jour yt-dlp au lancement (`-U`) : c'est le composant qui casse le
/// plus souvent, YouTube changeant en permanence. Non bloquant côté UI.
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<UpdateResult, String> {
    let exe = ytdlp_path(&app);
    if !exe.exists() {
        return Err("yt-dlp absent".into());
    }
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.arg("-U");
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = cmd.output().await.map_err(|e| format!("mise à jour yt-dlp : {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let updated = text.contains("Updated yt-dlp to");
    // version = token "stable@2026.xx.xx" de la ligne pertinente
    let version = text
        .lines()
        .find(|l| l.contains("Updated yt-dlp to") || l.contains("up to date"))
        .and_then(|l| l.split_whitespace().find(|w| w.contains('@')))
        .unwrap_or("")
        .trim_matches(|c| c == '(' || c == ')')
        .to_string();
    Ok(UpdateResult { updated, version })
}

#[cfg(test)]
mod tests {
    use super::extract_from_zip;
    use std::io::Write;

    fn make_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }

    #[tokio::test]
    async fn extracts_wanted_files_ignoring_others() {
        let dir = std::env::temp_dir().join(format!("fs-test-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("test.zip");
        make_zip(&zip_path, &[
            ("ffmpeg-essentials/bin/ffmpeg.exe", b"fake ffmpeg binary"),
            ("ffmpeg-essentials/bin/ffprobe.exe", b"fake ffprobe binary"),
            ("ffmpeg-essentials/LICENSE", b"license text, not wanted"),
        ]);

        let result = extract_from_zip(zip_path.clone(), dir.clone(), &["ffmpeg.exe", "ffprobe.exe"]).await;
        assert!(result.is_ok(), "expected success, got {result:?}");
        assert!(dir.join("ffmpeg.exe").exists());
        assert!(dir.join("ffprobe.exe").exists());
        assert!(!dir.join("LICENSE").exists(), "unwanted entries must not be extracted");
        assert_eq!(std::fs::read(dir.join("ffmpeg.exe")).unwrap(), b"fake ffmpeg binary");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn errors_clearly_when_a_wanted_file_is_missing() {
        let dir = std::env::temp_dir().join(format!("fs-test-zip-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("test.zip");
        make_zip(&zip_path, &[("bin/ffmpeg.exe", b"present")]);

        let result = extract_from_zip(zip_path.clone(), dir.clone(), &["ffmpeg.exe", "ffprobe.exe"]).await;
        let err = result.expect_err("ffprobe.exe is missing from the archive, must error");
        assert!(err.contains("ffprobe.exe"), "error should name the missing file: {err}");
        // ce qui a été trouvé est quand même extrait (pas de rollback nécessaire,
        // le prochain essai skip les fichiers déjà présents comme setup_tools le fait)
        assert!(dir.join("ffmpeg.exe").exists());

        std::fs::remove_dir_all(&dir).ok();
    }
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
        extract_from_zip(zip_path.clone(), bin.clone(), &["ffmpeg.exe", "ffprobe.exe"]).await?;
        std::fs::remove_file(&zip_path).ok();
    }

    let deno = deno_path(&app);
    if !deno.exists() {
        emit_setup(&app, "deno (runtime YouTube)", 0.0);
        let zip_path = bin.join("deno.zip");
        download_file(&app, DENO_URL, &zip_path, "deno (runtime YouTube)").await?;
        emit_setup(&app, "extraction", 0.0);
        extract_from_zip(zip_path.clone(), bin.clone(), &["deno.exe"]).await?;
        std::fs::remove_file(&zip_path).ok();
    }

    emit_setup(&app, "terminé", 100.0);
    Ok(tools_status(app))
}
