//! Cookies de connexion (Netscape cookies.txt) — Facebook/Instagram/Threads
//! exigent une session pour la plupart des contenus. L'utilisateur importe
//! un cookies.txt exporté du navigateur ; stocké en local (app locale, pas
//! de chiffrement — cohérent avec le reste de l'app), passé à yt-dlp via
//! --cookies quand présent.

use serde::Serialize;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager};

const MAX_COOKIES_BYTES: u64 = 512 * 1024;

fn cookies_file(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("app_data_dir indisponible");
    std::fs::create_dir_all(&dir).ok();
    dir.join("cookies.txt")
}

/// Chemin des cookies s'ils existent — utilisé par jobs.rs pour --cookies.
pub(crate) fn cookies_path_if_present(app: &AppHandle) -> Option<PathBuf> {
    let p = cookies_file(app);
    p.exists().then_some(p)
}

fn validate_cookies_text(text: &str) -> Result<(), String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("fichier vide".into());
    }
    if trimmed.len() as u64 > MAX_COOKIES_BYTES {
        return Err("fichier trop volumineux (512 Ko max)".into());
    }
    if trimmed.contains('\0') {
        return Err("fichier binaire, cookies.txt attendu".into());
    }
    let has_header = trimmed
        .lines()
        .take(5)
        .any(|l| l.trim_start().starts_with("# Netscape") || l.trim_start().starts_with("# HTTP Cookie File"));
    let has_cookie_line = trimmed.lines().any(|l| l.split('\t').count() == 7);
    if !has_header && !has_cookie_line {
        return Err("format cookies.txt (Netscape) non reconnu".into());
    }
    Ok(())
}

#[derive(Serialize)]
pub struct CookiesStatus {
    pub present: bool,
    pub updated_at: Option<String>,
}

#[tauri::command]
pub fn cookies_status(app: AppHandle) -> CookiesStatus {
    let p = cookies_file(&app);
    match std::fs::metadata(&p) {
        Ok(meta) if p.exists() => {
            let updated_at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string());
            CookiesStatus { present: true, updated_at }
        }
        _ => CookiesStatus { present: false, updated_at: None },
    }
}

/// Lit le fichier choisi par l'utilisateur (dialogue natif côté frontend),
/// valide le format, puis le copie dans le dossier de données de l'app.
#[tauri::command]
pub fn set_cookies_file(app: AppHandle, source_path: String) -> Result<CookiesStatus, String> {
    let text = std::fs::read_to_string(&source_path).map_err(|e| format!("lecture impossible : {e}"))?;
    validate_cookies_text(&text)?;
    std::fs::write(cookies_file(&app), text).map_err(|e| format!("écriture impossible : {e}"))?;
    Ok(cookies_status(app))
}

#[tauri::command]
pub fn clear_cookies_file(app: AppHandle) -> Result<(), String> {
    let p = cookies_file(&app);
    if p.exists() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_cookies_text;

    #[test]
    fn accepts_netscape_header() {
        assert!(validate_cookies_text("# Netscape HTTP Cookie File\n.facebook.com\tTRUE\t/\tTRUE\t0\tc_user\t1\n").is_ok());
    }

    #[test]
    fn accepts_tab_separated_line_without_header() {
        assert!(validate_cookies_text(".instagram.com\tTRUE\t/\tTRUE\t0\tsessionid\tabc").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_cookies_text("   ").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(validate_cookies_text("just some random text").is_err());
    }

    #[test]
    fn rejects_binary() {
        assert!(validate_cookies_text("# Netscape HTTP Cookie File\n\0garbage").is_err());
    }

    #[test]
    fn rejects_oversized() {
        let big = format!("# Netscape HTTP Cookie File\n{}", "a".repeat(600 * 1024));
        assert!(validate_cookies_text(&big).is_err());
    }
}
