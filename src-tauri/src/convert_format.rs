//! Formats et arguments ffmpeg pour la conversion / extraction / fusion audio+vidéo.
//! Miroir des tables de convert.js côté web (même choix de codecs par conteneur).

pub const CONVERT_VIDEO_CONTAINERS: [&str; 8] =
    ["mp4", "mkv", "webm", "mov", "avi", "wmv", "flv", "gif"];
pub const MUX_CONTAINERS: [&str; 7] = ["mp4", "mkv", "webm", "mov", "avi", "wmv", "flv"];
pub const CONVERT_AUDIO_FORMATS: [&str; 7] = ["mp3", "m4a", "aac", "opus", "flac", "wav", "ogg"];

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Piste audio à appliquer pour un conteneur vidéo donné (conversion complète ou fusion).
pub fn audio_for_container(container: &str) -> Option<Vec<String>> {
    Some(match container {
        "mp4" | "mkv" | "mov" | "flv" => s(&["-c:a", "aac", "-b:a", "192k"]),
        "avi" => s(&["-c:a", "libmp3lame", "-b:a", "192k"]),
        "wmv" => s(&["-c:a", "wmav2", "-b:a", "192k"]),
        "webm" => s(&["-c:a", "libopus", "-b:a", "160k"]),
        _ => return None,
    })
}

/// Arguments vidéo complets (codec vidéo + piste audio) pour une conversion vers `container`.
pub fn video_args(container: &str) -> Option<Vec<String>> {
    if container == "gif" {
        return Some(s(&["-vf", "fps=12,scale=480:-1:flags=lanczos", "-loop", "0"]));
    }
    let video: Vec<String> = match container {
        "mp4" | "mkv" | "mov" | "flv" => s(&["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"]),
        "avi" => s(&["-c:v", "mpeg4", "-qscale:v", "4"]),
        "wmv" => s(&["-c:v", "wmv2", "-b:v", "4M"]),
        "webm" => s(&["-c:v", "libvpx-vp9", "-crf", "32", "-b:v", "0"]),
        _ => return None,
    };
    let mut args = video;
    args.extend(audio_for_container(container)?);
    Some(args)
}

/// Arguments de conversion audio (ou d'extraction depuis une vidéo, via -vn en amont).
pub fn audio_args(format: &str) -> Option<Vec<String>> {
    Some(match format {
        "mp3" => s(&["-c:a", "libmp3lame", "-q:a", "2"]),
        "m4a" | "aac" => s(&["-c:a", "aac", "-b:a", "256k"]),
        "opus" => s(&["-c:a", "libopus", "-b:a", "192k"]),
        "flac" => s(&["-c:a", "flac"]),
        "wav" => s(&["-c:a", "pcm_s16le"]),
        "ogg" => s(&["-c:a", "libvorbis", "-q:a", "6"]),
        _ => return None,
    })
}
