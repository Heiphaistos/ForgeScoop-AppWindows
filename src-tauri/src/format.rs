//! Traduction des formats ForgeScoop en arguments yt-dlp.
//! Même grammaire que la version web :
//!   v-<qualité>-<conteneur>-<audio|mute> | a-<format audio> | s-<qualité>-<conteneur>-<format audio>

const QUALITIES: [&str; 10] = [
    "best", "4320p", "2160p", "1440p", "1080p", "720p", "480p", "360p", "240p", "144p",
];
const CONTAINERS: [&str; 7] = ["mp4", "mkv", "webm", "mov", "avi", "wmv", "flv"];
const AUDIO_FORMATS: [&str; 5] = ["mp3", "m4a", "opus", "flac", "wav"];

fn video_selector(quality: &str, container: &str, with_audio: bool) -> String {
    let h = if quality == "best" {
        String::new()
    } else {
        format!("[height<={}]", quality.trim_end_matches('p'))
    };
    let pref_webm = container == "webm";
    let pref = if pref_webm { "[ext=webm]" } else { "[vcodec^=avc]" };
    if !with_audio {
        return format!("bv*{pref}{h}/bv*{h}/b{h}/b");
    }
    let pref_audio = if pref_webm { "[ext=webm]" } else { "[acodec^=mp4a]" };
    format!("bv*{pref}{h}+ba{pref_audio}/bv*{h}+ba/b{h}/b")
}

fn container_args(container: &str) -> Vec<String> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    match container {
        "mkv" => s(&["--merge-output-format", "mkv", "--remux-video", "mkv"]),
        "mp4" | "webm" => vec![
            "--merge-output-format".into(),
            format!("{container}/mkv"),
            "--recode-video".into(),
            container.into(),
        ],
        "mov" => s(&["--merge-output-format", "mp4/mkv", "--remux-video", "mov"]),
        other => vec![
            "--merge-output-format".into(),
            "mp4/mkv".into(),
            "--recode-video".into(),
            other.into(),
        ],
    }
}

/// Valide un format et retourne les arguments yt-dlp correspondants.
pub fn format_args(format: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = format.split('-').collect();
    match parts.as_slice() {
        ["v", quality, container, sound] if QUALITIES.contains(quality)
            && CONTAINERS.contains(container)
            && (*sound == "audio" || *sound == "mute") =>
        {
            let mut args = vec!["-f".to_string(), video_selector(quality, container, *sound == "audio")];
            args.extend(container_args(container));
            Some(args)
        }
        ["a", audio] if AUDIO_FORMATS.contains(audio) => Some(vec![
            "-f".into(), "ba/b".into(),
            "-x".into(), "--audio-format".into(), (*audio).into(),
            "--audio-quality".into(), "0".into(),
        ]),
        ["s", quality, container, audio] if QUALITIES.contains(quality)
            && CONTAINERS.contains(container)
            && AUDIO_FORMATS.contains(audio) =>
        {
            let mut args = vec!["-f".to_string(), video_selector(quality, container, true)];
            args.extend(container_args(container));
            args.extend(["-x".into(), "-k".into(), "--audio-format".into(), (*audio).into(), "--audio-quality".into(), "0".into()]);
            Some(args)
        }
        _ => None,
    }
}
