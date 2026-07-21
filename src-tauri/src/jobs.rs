//! File de téléchargement : spawn yt-dlp, progression en events Tauri,
//! inspection de playlist, renommage manuel + IA (Pollinations, gratuit).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::format::format_args;
use crate::tools::{deno_path, ffmpeg_dir, ytdlp_path};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Default)]
pub struct JobRegistry(Mutex<HashMap<String, u32>>); // id -> pid

#[derive(Serialize, Clone)]
pub(crate) struct ProgressEvent {
    pub id: String,
    pub progress: f64,
    pub speed: String,
    pub eta: String,
    pub item_index: Option<u32>,
    pub item_count: Option<u32>,
    pub item_title: Option<String>,
}

#[derive(Serialize, Clone)]
struct MetaEvent {
    id: String,
    title: String,
    upload_date: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct DoneEvent {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
    pub files: Vec<String>,
}

/// Enregistre le PID d'un job (partagé par les téléchargements et les conversions,
/// pour que cancel_job()/kill_all() fonctionnent uniformément).
pub(crate) fn register_pid(registry: &JobRegistry, id: String, pid: u32) {
    registry.0.lock().unwrap().insert(id, pid);
}
pub(crate) fn unregister_pid(registry: &JobRegistry, id: &str) {
    registry.0.lock().unwrap().remove(id);
}

#[derive(Serialize)]
pub struct PlaylistEntry {
    pub index: u32,
    pub title: String,
    pub duration: Option<f64>,
}

#[derive(Serialize)]
pub struct InspectResult {
    pub is_playlist: bool,
    pub title: String,
    pub entries: Vec<PlaylistEntry>,
}

fn base_command(app: &AppHandle) -> Command {
    let mut cmd = Command::new(ytdlp_path(app));
    // Sans console, Python encode stdout dans la locale (cp1252) : un titre
    // accentué produit des octets non-UTF-8, le lecteur du pipe meurt, yt-dlp
    // écrit dans un pipe fermé → EINVAL ("[Errno 22] Invalid argument").
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUTF8", "1");
    // runtime JS exigé par yt-dlp pour YouTube (auto-installé par tools.rs)
    let deno = deno_path(app);
    if deno.exists() {
        cmd.arg("--js-runtimes");
        cmd.arg(format!("deno:{}", deno.to_string_lossy()));
    }
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn clean_youtube_url(raw: &str) -> String {
    // Mix radio YouTube (list=RD*) : yt-dlp ne l'étend que depuis une watch
    // URL — un lien /playlist?list=RD… est reconverti via la vidéo graine
    // encodée dans l'identifiant (RD/RDMM/RDAMVM + id 11 caractères)
    if let Ok(mut u) = url::Url::parse(raw) {
        let host = u.host_str().unwrap_or("").to_lowercase();
        if host.ends_with("youtube.com") {
            let list = u
                .query_pairs()
                .find(|(k, _)| k == "list")
                .map(|(_, v)| v.into_owned());
            let has_v = u.query_pairs().any(|(k, _)| k == "v");
            if let Some(list) = list.filter(|l| l.starts_with("RD")) {
                if !has_v {
                    let seed = list
                        .strip_prefix("RDAMVM")
                        .or_else(|| list.strip_prefix("RDMM"))
                        .or_else(|| list.strip_prefix("RD"))
                        .filter(|s| s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));
                    if let Some(seed) = seed {
                        let seed = seed.to_string();
                        u.set_path("/watch");
                        let kept: Vec<(String, String)> = u
                            .query_pairs()
                            .filter(|(k, _)| k != "index")
                            .map(|(k, v)| (k.into_owned(), v.into_owned()))
                            .collect();
                        u.query_pairs_mut().clear();
                        for (k, v) in kept {
                            u.query_pairs_mut().append_pair(&k, &v);
                        }
                        u.query_pairs_mut().append_pair("v", &seed);
                    }
                }
            }
        }
        return u.to_string();
    }
    raw.to_string()
}

pub(crate) fn is_mix_url(url: &str) -> bool {
    // mix infini à graine seulement — les playlists éditoriales RDCLAK… sont finies
    url::Url::parse(url)
        .map(|u| {
            u.query_pairs().any(|(k, v)| {
                k == "list"
                    && v.strip_prefix("RDAMVM")
                        .or_else(|| v.strip_prefix("RDMM"))
                        .or_else(|| v.strip_prefix("RD"))
                        .map(|s| s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn mix_watch_url_kept() {
        let out = super::clean_youtube_url("https://www.youtube.com/watch?v=5WMyJCivfDs&list=RDMMwH13tLs2w64&index=5");
        assert!(out.contains("v=5WMyJCivfDs") && out.contains("list=RDMMwH13tLs2w64"));
    }

    #[test]
    fn mix_playlist_url_rebuilt() {
        let out = super::clean_youtube_url("https://www.youtube.com/playlist?list=RDdQw4w9WgXcQ");
        assert!(out.contains("/watch") && out.contains("v=dQw4w9WgXcQ") && out.contains("list=RDdQw4w9WgXcQ"));
    }

    #[test]
    fn mix_detected() {
        assert!(super::is_mix_url("https://www.youtube.com/watch?v=a&list=RDdQw4w9WgXcQ"));
        assert!(!super::is_mix_url("https://www.youtube.com/watch?v=a&list=PLx"));
        // playlist éditoriale finie : pas un mix, pas de cap
        assert!(!super::is_mix_url("https://www.youtube.com/playlist?list=RDCLAK5uy_kb7EBi6y3GrtJri4_ZH56Ms786DFEimbM"));
    }
}

fn validate_url(raw: &str) -> Result<String, String> {
    let cleaned = clean_youtube_url(raw.trim());
    let u = url::Url::parse(&cleaned).map_err(|_| "URL invalide".to_string())?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err("seuls http/https sont acceptés".into());
    }
    Ok(cleaned)
}

/// Sous-titres : mode srt/vtt (fichiers, auto-générés inclus) ou embed
/// (incrustés — mp4/mkv/webm uniquement, limite yt-dlp).
fn subs_args(mode: &str, langs: &str, format: &str) -> Result<Vec<String>, String> {
    if !langs.chars().all(|c| c.is_ascii_alphanumeric() || ",.*-".contains(c)) || langs.is_empty() || langs.len() > 60 {
        return Err("langues de sous-titres invalides".into());
    }
    match mode {
        "embed" => {
            let container = format.split('-').nth(2).unwrap_or("");
            if !matches!(container, "mp4" | "mkv" | "webm") {
                return Err("sous-titres incrustés : mp4/mkv/webm uniquement".into());
            }
            Ok(vec!["--embed-subs".into(), "--sub-langs".into(), langs.into()])
        }
        "srt" | "vtt" => Ok(vec![
            "--write-subs".into(), "--write-auto-subs".into(),
            "--sub-langs".into(), langs.into(),
            "--convert-subs".into(), mode.into(),
        ]),
        _ => Err("mode de sous-titres invalide".into()),
    }
}

/// Découpe "start-end" en secondes ("inf" = fin) — coupe aux keyframes.
fn section_args(section: &str) -> Result<Vec<String>, String> {
    let (start, end) = section.split_once('-').ok_or("découpe invalide")?;
    let s: f64 = start.parse().map_err(|_| "découpe : début invalide")?;
    let e = if end == "inf" { f64::INFINITY } else { end.parse().map_err(|_| "découpe : fin invalide")? };
    if s < 0.0 || e <= s || s > 259_200.0 {
        return Err("découpe : horodatages invalides".into());
    }
    Ok(vec!["--download-sections".into(), format!("*{start}-{end}")])
}

#[tauri::command]
pub fn default_download_dir(app: AppHandle) -> String {
    use tauri::Manager;
    let dir = app
        .path()
        .download_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("ForgeScoop");
    dir.to_string_lossy().to_string()
}

#[tauri::command]
pub async fn inspect_url(app: AppHandle, url: String) -> Result<InspectResult, String> {
    let url = validate_url(&url)?;
    let out = base_command(&app)
        .args(["--no-colors", "--flat-playlist", "--skip-download", "-J", "--", &url])
        .output()
        .await
        .map_err(|e| format!("yt-dlp : {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rev().find(|l| l.contains("ERROR")).unwrap_or("analyse impossible");
        return Err(line.chars().take(300).collect());
    }
    let data: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|_| "réponse yt-dlp illisible")?;
    if data["_type"] == "playlist" {
        let entries = data["entries"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(500)
            .enumerate()
            .map(|(i, e)| PlaylistEntry {
                index: i as u32 + 1,
                title: e["title"].as_str().unwrap_or("Sans titre").to_string(),
                duration: e["duration"].as_f64(),
            })
            .collect();
        Ok(InspectResult {
            is_playlist: true,
            title: data["title"].as_str().unwrap_or("Playlist").to_string(),
            entries,
        })
    } else {
        Ok(InspectResult {
            is_playlist: false,
            title: data["title"].as_str().unwrap_or(&url).to_string(),
            entries: vec![],
        })
    }
}

#[tauri::command]
pub async fn start_job(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    id: String,
    url: String,
    format: String,
    dest: String,
    playlist: bool,
    items: Option<Vec<u32>>,
    subs_mode: Option<String>,
    subs_langs: Option<String>,
    section: Option<String>,
) -> Result<(), String> {
    let url = validate_url(&url)?;
    let fmt_args = format_args(&format).ok_or("format invalide")?;
    let mut extra_args: Vec<String> = Vec::new();
    if let Some(mode) = subs_mode.as_deref().filter(|m| !m.is_empty() && *m != "none") {
        extra_args.extend(subs_args(mode, subs_langs.as_deref().unwrap_or("fr,en"), &format)?);
    }
    if let Some(sec) = section.as_deref().filter(|s| !s.is_empty()) {
        extra_args.extend(section_args(sec)?);
    }
    let dest_dir = PathBuf::from(&dest);
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("dossier de destination : {e}"))?;

    let job_dir = dest_dir.join(format!(".fs-{}", &id[..8]));
    std::fs::create_dir_all(&job_dir).map_err(|e| e.to_string())?;

    let template = if playlist {
        "%(playlist_index|)s%(playlist_index&-|)s%(title).170B.%(ext)s"
    } else {
        "%(title).180B.%(ext)s"
    };

    let mut args: Vec<String> = vec![
        "--no-colors".into(), "--newline".into(), "--progress".into(),
        "--restrict-filenames".into(), "--no-part".into(), "--no-mtime".into(),
        // [Errno 22]/verrous transitoires (antivirus, reprise) : réessayer au lieu d'échouer
        "--retries".into(), "10".into(),
        "--file-access-retries".into(), "10".into(),
        "--fragment-retries".into(), "10".into(),
        "--ffmpeg-location".into(), ffmpeg_dir(&app).to_string_lossy().into_owned(),
        "--progress-template".into(),
        "download:FSPROG|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(info.playlist_index|)s|%(info.playlist_count|)s|%(info.title|)s".into(),
        "--print".into(), "before_dl:FSMETA|%(title)s|%(upload_date|)s".into(),
        "--no-simulate".into(),
        "-o".into(), job_dir.join(template).to_string_lossy().into_owned(),
    ];
    if playlist {
        args.push("--yes-playlist".into());
        let has_selection = items.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        if has_selection {
            let mut sorted: Vec<u32> = items.clone().unwrap_or_default();
            sorted.sort_unstable();
            sorted.dedup();
            args.push("--playlist-items".into());
            args.push(sorted.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(","));
        } else if is_mix_url(&url) {
            // Mix radio sans sélection explicite : liste quasi infinie → cap
            args.push("--playlist-items".into());
            args.push("1-50".into());
        }
    } else {
        args.push("--no-playlist".into());
    }
    args.extend(extra_args);
    args.extend(fmt_args);
    args.push("--".into());
    args.push(url);

    let mut child = base_command(&app)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("lancement yt-dlp : {e}"))?;

    if let Some(pid) = child.id() {
        registry.0.lock().unwrap().insert(id.clone(), pid);
    }

    let stdout = child.stdout.take().ok_or("stdout indisponible")?;
    let stderr = child.stderr.take().ok_or("stderr indisponible")?;
    let app2 = app.clone();
    let id2 = id.clone();

    // lecture de la progression — tolérante aux octets non-UTF-8 (titres dans
    // une locale exotique) : ne jamais fermer le pipe tant que yt-dlp écrit
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buf: Vec<u8> = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim_end_matches(['\r', '\n']);
            if let Some(rest) = line.strip_prefix("FSMETA|") {
                let mut it = rest.splitn(2, '|');
                let title = it.next().unwrap_or("").trim().to_string();
                let upload_date = it.next().unwrap_or("").trim().to_string();
                app2.emit("job-meta", MetaEvent { id: id2.clone(), title, upload_date }).ok();
            } else if let Some(rest) = line.strip_prefix("FSPROG|") {
                let parts: Vec<&str> = rest.split('|').collect();
                if parts.len() >= 5 {
                    let progress = parts[0].trim().trim_end_matches('%').parse::<f64>().unwrap_or(0.0);
                    let ev = ProgressEvent {
                        id: id2.clone(),
                        progress,
                        speed: parts[1].trim().to_string(),
                        eta: parts[2].trim().to_string(),
                        item_index: parts[3].trim().parse().ok(),
                        item_count: parts[4].trim().parse().ok(),
                        item_title: (parts.len() > 5).then(|| parts[5..].join("|").trim().to_string()),
                    };
                    app2.emit("job-progress", ev).ok();
                }
            }
        }
    });

    // stderr : garder la dernière erreur (lecture tolérante, comme stdout)
    let err_buf = std::sync::Arc::new(Mutex::new(String::new()));
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
            if line.contains("ERROR") {
                *err_buf2.lock().unwrap() = line.to_string();
            }
        }
    });

    let status = child.wait().await.map_err(|e| e.to_string())?;

    // fin de job : déplacer les fichiers du dossier temporaire vers la destination
    let mut result_files: Vec<String> = Vec::new();
    let mut error: Option<String> = None;
    if status.success() {
        if let Ok(entries) = std::fs::read_dir(&job_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                // purge des flux intermédiaires du mode split
                if name.ends_with(".part") || name.ends_with(".ytdl") {
                    std::fs::remove_file(entry.path()).ok();
                    continue;
                }
                if regex_fragment(&name) {
                    std::fs::remove_file(entry.path()).ok();
                    continue;
                }
                let mut target = dest_dir.join(&name);
                let mut n = 1;
                while target.exists() {
                    let stem = PathBuf::from(&name);
                    let ext = stem.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
                    let base = stem.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name.clone());
                    target = dest_dir.join(format!("{base} ({n}).{ext}"));
                    n += 1;
                }
                if std::fs::rename(entry.path(), &target).is_ok() {
                    result_files.push(target.to_string_lossy().into_owned());
                }
            }
        }
        if result_files.is_empty() {
            error = Some("aucun fichier produit".into());
        }
    } else {
        let e = err_buf.lock().unwrap().clone();
        error = Some(if e.is_empty() { "téléchargement annulé ou échoué".into() } else { e });
    }
    std::fs::remove_dir_all(&job_dir).ok();

    registry.0.lock().unwrap().remove(&id);
    app.emit("job-done", DoneEvent { id, ok: error.is_none(), error, files: result_files }).ok();
    Ok(())
}

fn regex_fragment(name: &str) -> bool {
    // équivalent de /\.f\d+\.\w+$/ sans dépendance regex
    if let Some(pos) = name.rfind(".f") {
        let rest = &name[pos + 2..];
        if let Some(dot) = rest.find('.') {
            return !rest[..dot].is_empty() && rest[..dot].chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

#[cfg(windows)]
fn taskkill(pid: u32) -> bool {
    let mut kill = std::process::Command::new("taskkill");
    kill.args(["/F", "/T", "/PID", &pid.to_string()]);
    use std::os::windows::process::CommandExt;
    kill.creation_flags(CREATE_NO_WINDOW);
    kill.status().map(|s| s.success()).unwrap_or(false)
}

/// Tue tous les yt-dlp encore actifs — appelé à la fermeture de l'app pour ne
/// laisser aucun orphelin écrire dans les fichiers repris au prochain lancement.
pub fn kill_all(registry: &JobRegistry) {
    let pids: Vec<u32> = registry.0.lock().unwrap().drain().map(|(_, pid)| pid).collect();
    #[cfg(windows)]
    for pid in pids {
        taskkill(pid);
    }
    #[cfg(not(windows))]
    let _ = pids;
}

#[tauri::command]
pub fn cancel_job(registry: State<'_, JobRegistry>, id: String) -> bool {
    if let Some(pid) = registry.0.lock().unwrap().remove(&id) {
        #[cfg(windows)]
        return taskkill(pid);
        #[cfg(not(windows))]
        let _ = pid;
    }
    false
}

fn spawn_explorer(arg: String) -> Result<(), String> {
    let mut cmd = std::process::Command::new("explorer.exe");
    cmd.arg(arg);
    // explorer.exe renvoie souvent un code != 0 même en cas de succès : ne pas vérifier
    cmd.spawn().map(|_| ()).map_err(|e| format!("explorateur Windows : {e}"))
}

/// Ouvre un fichier avec l'application associée (via l'Explorateur Windows).
#[tauri::command]
pub fn open_file(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err("fichier introuvable (déplacé ou supprimé ?)".into());
    }
    spawn_explorer(p.to_string_lossy().into_owned())
}

/// Révèle un fichier sélectionné dans l'Explorateur Windows.
#[tauri::command]
pub fn show_in_folder(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("fichier introuvable (déplacé ou supprimé ?)".into());
    }
    spawn_explorer(format!("/select,{}", p.to_string_lossy()))
}

#[tauri::command]
pub fn rename_file(path: String, new_base: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("fichier introuvable".into());
    }
    let cleaned: String = new_base
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) || (c as u32) < 32 { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        return Err("nom invalide".into());
    }
    let ext = p.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let target = p.with_file_name(if ext.is_empty() { cleaned.clone() } else { format!("{cleaned}.{ext}") });
    std::fs::rename(&p, &target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

#[derive(Deserialize)]
struct PollinationsChoice {
    message: PollinationsMsg,
}
#[derive(Deserialize)]
struct PollinationsMsg {
    content: String,
}
#[derive(Deserialize)]
struct PollinationsResp {
    choices: Vec<PollinationsChoice>,
}

/// Renommage IA via Pollinations (gratuit, sans clé) : "Titre (date) [format]".
#[tauri::command]
pub async fn ai_rename(
    path: String,
    title: String,
    upload_date: String,
    format: String,
    url: String,
) -> Result<String, String> {
    let date = if upload_date.len() == 8 && upload_date.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &upload_date[..4], &upload_date[4..6], &upload_date[6..8])
    } else {
        String::new()
    };
    let system = "Tu nettoies des titres de vidéos/musiques téléchargées pour en faire des noms de fichiers. \
        Règles : retire le clickbait, les emojis, les hashtags, les mentions de chaîne inutiles ; \
        garde Artiste - Titre pour la musique ; conserve la langue d'origine ; \
        aucun caractère interdit dans un nom de fichier. \
        Réponds UNIQUEMENT avec le nom final, sans extension, sans guillemets.";
    let user = format!(
        "Titre brut : {title}\nURL : {url}\nDate de sortie : {}\nFormat : {format}\n\nCompose le nom : \"Titre nettoyé{} [format]\".",
        if date.is_empty() { "inconnue" } else { &date },
        if date.is_empty() { "" } else { " (date)" }
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp: PollinationsResp = client
        .post("https://text.pollinations.ai/openai")
        .json(&serde_json::json!({
            "model": "openai",
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "max_tokens": 200,
            "temperature": 0.2
        }))
        .send()
        .await
        .map_err(|e| format!("service IA injoignable : {e}"))?
        .json()
        .await
        .map_err(|_| "réponse IA illisible".to_string())?;

    let name = resp
        .choices
        .first()
        .map(|c| c.message.content.trim().trim_matches('"').lines().next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or("l'IA n'a pas renvoyé de nom")?;

    rename_file(path, name)
}
