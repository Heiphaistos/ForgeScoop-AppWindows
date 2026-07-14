# ForgeScoop pour Windows

Téléchargeur vidéo & audio natif pour **Windows 10/11** — la version bureau de [ForgeScoop](https://github.com/Heiphaistos/ForgeScoop). YouTube, TikTok, Instagram, Facebook, X et 1000+ sites via yt-dlp.

## Fonctionnalités

- **Multi-sources** : tout site supporté par yt-dlp, URLs en lot
- **Formats** : MP4, MKV, WebM, MOV, AVI, WMV, FLV × qualités 144p → 8K, avec ou sans piste audio
- **Audio seul** : MP3, M4A (AAC), Opus, FLAC, WAV — ou vidéo + audio séparés
- **Playlists** : sélecteur de titres (cases à cocher), file d'attente visible en temps réel
- **Renommage** : manuel ou IA (« Titre (date de sortie) [format] », gratuit, sans clé)
- **Zéro dépendance à installer** : yt-dlp et FFmpeg sont téléchargés automatiquement au premier lancement
- **15 thèmes + couleur libre**, 3 dispositions, logo 3D animé
- Fichiers enregistrés directement dans le dossier de votre choix (défaut : `Téléchargements\ForgeScoop`)

## Installation

Téléchargez `ForgeScoop_x64-setup.exe` depuis les [Releases](../../releases) et lancez-le (installation par utilisateur, pas de droits admin requis).

## Stack

Tauri v2 + Rust (tokio, reqwest) · Vue 3 + Vite · yt-dlp + FFmpeg (auto-installés dans `%APPDATA%\org.heiphaistos.forgescoop\bin`)

## Développement

```bash
npm install
cargo tauri dev     # développement
cargo tauri build   # installeur NSIS dans src-tauri/target/release/bundle/nsis/
```

## Note légale

Outil personnel : ne téléchargez que des contenus que vous avez le droit de récupérer (vos propres contenus, contenus libres, copie privée selon votre législation).
