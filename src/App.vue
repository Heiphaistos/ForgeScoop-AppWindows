<script setup>
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';

/* ===== Thèmes & disposition (identique au site) ===== */
const THEMES = [
  { id: 'forge', label: 'Forge', color: '#f78166' },
  { id: 'ocean', label: 'Océan', color: '#4cc2ff' },
  { id: 'violet', label: 'Violet', color: '#b18aff' },
  { id: 'emeraude', label: 'Émeraude', color: '#34d399' },
  { id: 'rose', label: 'Rose', color: '#f472b6' },
  { id: 'crimson', label: 'Crimson', color: '#f43f5e' },
  { id: 'ambre', label: 'Ambre', color: '#f5b428' },
  { id: 'cyan', label: 'Cyan', color: '#22d3ee' },
  { id: 'minuit', label: 'Minuit', color: '#6366f1' },
  { id: 'acier', label: 'Acier', color: '#94a3b8' },
  { id: 'sunset', label: 'Sunset', color: '#fb923c' },
  { id: 'foret', label: 'Forêt', color: '#86c232' },
  { id: 'clair', label: 'Clair', color: '#e8e2d8' },
  { id: 'lavande-clair', label: 'Lavande clair', color: '#7c5cd6' },
  { id: 'menthe-clair', label: 'Menthe clair', color: '#0d9464' }
];
const LAYOUTS = [
  { id: 'comfort', label: 'Confort', desc: 'Cartes espacées, tous les détails' },
  { id: 'compact', label: 'Compact', desc: 'Lignes serrées, plus de contenu à l’écran' },
  { id: 'grid', label: 'Grille', desc: 'Deux colonnes côte à côte' }
];
const theme = ref(localStorage.getItem('fs-theme') || 'forge');
const customColor = ref(localStorage.getItem('fs-custom-color') || '#f78166');
const layout = ref(localStorage.getItem('fs-layout') || 'comfort');
const settingsOpen = ref(false);
const aboutOpen = ref(false);

function hexToRgb(hex) {
  const m = hex.match(/^#?([0-9a-f]{6})$/i);
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [n >> 16, (n >> 8) & 255, n & 255];
}
const mix = (rgb, t, k) => rgb.map((v, i) => Math.round(v + (t[i] - v) * k));
const toHex = (rgb) => `#${rgb.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
function applyCustom(hex) {
  const rgb = hexToRgb(hex);
  if (!rgb) return;
  const root = document.documentElement;
  root.dataset.theme = 'forge';
  const vars = {
    '--accent': toHex(rgb),
    '--accent2': toHex(mix(rgb, [255, 255, 255], 0.35)),
    '--accent-strong': toHex(mix(rgb, [0, 0, 0], 0.25)),
    '--border-focus': toHex(rgb),
    '--on-accent': (rgb[0] * 299 + rgb[1] * 587 + rgb[2] * 114) / 1000 > 150 ? '#101318' : '#ffffff',
    '--glow': `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, .13)`,
    '--bg0': toHex(mix(rgb, [8, 10, 14], 0.94)),
    '--bg1': toHex(mix(rgb, [14, 17, 23], 0.93)),
    '--bg2': toHex(mix(rgb, [21, 25, 34], 0.92)),
    '--border': toHex(mix(rgb, [38, 45, 61], 0.85))
  };
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
}
function clearCustom() {
  const root = document.documentElement;
  for (const k of ['--accent', '--accent2', '--accent-strong', '--border-focus', '--on-accent', '--glow', '--bg0', '--bg1', '--bg2', '--border']) root.style.removeProperty(k);
}
function setTheme(id) {
  theme.value = id;
  localStorage.setItem('fs-theme', id);
  if (id === 'custom') applyCustom(customColor.value);
  else { clearCustom(); document.documentElement.dataset.theme = id; }
}
function setCustomColor(e) {
  customColor.value = e.target.value;
  localStorage.setItem('fs-custom-color', customColor.value);
  setTheme('custom');
}
function setLayout(id) { layout.value = id; localStorage.setItem('fs-layout', id); }
setTheme(theme.value);

/* ===== Outils (yt-dlp / ffmpeg) ===== */
const toolsReady = ref(null); // null = vérification, false = installation, true = prêt
const setupStep = ref('');
const setupProgress = ref(0);
const setupError = ref('');
const ytdlpNote = ref(''); // note transitoire après la mise à jour auto

/* ===== Téléchargements ===== */
const MODES = [
  { value: 'video', label: '🎬 Vidéo' },
  { value: 'audio', label: '🎵 Audio seul' },
  { value: 'split', label: '🎬+🎵 Vidéo + audio séparés' }
];
const QUALITIES = [
  { value: 'best', label: 'Meilleure qualité' },
  { value: '4320p', label: '4320p (8K)' }, { value: '2160p', label: '2160p (4K)' },
  { value: '1440p', label: '1440p (2K)' }, { value: '1080p', label: '1080p (Full HD)' },
  { value: '720p', label: '720p (HD)' }, { value: '480p', label: '480p' },
  { value: '360p', label: '360p' }, { value: '240p', label: '240p' }, { value: '144p', label: '144p' }
];
const CONTAINERS = [
  { value: 'mp4', label: 'MP4 (universel)' }, { value: 'mkv', label: 'MKV' },
  { value: 'webm', label: 'WebM' }, { value: 'mov', label: 'MOV (Apple)' },
  { value: 'avi', label: 'AVI (réencodé)' }, { value: 'wmv', label: 'WMV (réencodé)' },
  { value: 'flv', label: 'FLV (réencodé)' }
];
const AUDIO_FORMATS = [
  { value: 'mp3', label: 'MP3' }, { value: 'm4a', label: 'M4A (AAC)' },
  { value: 'opus', label: 'Opus' }, { value: 'flac', label: 'FLAC (sans perte)' }, { value: 'wav', label: 'WAV' }
];
const STATUS_LABELS = { pending: 'en attente', running: 'en cours', done: 'terminé', error: 'erreur', canceled: 'annulé' };

const urlsText = ref('');
const mode = ref('video');
const quality = ref('best');
const container = ref('mp4');
const withSound = ref(true);
const audioFormat = ref('mp3');
const submitError = ref('');
const inspecting = ref(false);
const subsMode = ref('none');
const subsLangs = ref('fr,en');
const cutStart = ref('');
const cutEnd = ref('');
const destDir = ref(localStorage.getItem('fs-dest') || '');
const picker = ref(null);
// file persistante : tout l'historique est conservé, les jobs interrompus
// (running/pending au moment de la fermeture) sont repris au démarrage
const jobs = ref(JSON.parse(localStorage.getItem('fs-jobs') || '[]'));
const busyRename = ref(new Set());
const expandedQueue = ref(new Set());
const unlisteners = [];

const format = computed(() => {
  if (mode.value === 'audio') return `a-${audioFormat.value}`;
  if (mode.value === 'split') return `s-${quality.value}-${container.value}-${audioFormat.value}`;
  return `v-${quality.value}-${container.value}-${withSound.value ? 'audio' : 'mute'}`;
});

/* sous-titres incrustés : seulement mp4/mkv/webm (limite yt-dlp) */
const embedOk = computed(() => mode.value !== 'audio' && ['mp4', 'mkv', 'webm'].includes(container.value));
watch(embedOk, (ok) => { if (!ok && subsMode.value === 'embed') subsMode.value = 'srt'; });

/* "1:20" / "0:45" / "80" → secondes, NaN si illisible */
function parseTime(raw) {
  const s = String(raw ?? '').trim();
  if (!s) return null;
  if (!/^(\d{1,3}:)?(\d{1,2}:)?\d{1,4}(\.\d{1,3})?$/.test(s)) return NaN;
  const parts = s.split(':').map(Number);
  if (parts.slice(1).some((n) => n >= 60)) return NaN;
  return parts.reduce((acc, n) => acc * 60 + n, 0);
}
/* section canonique "start-end" en secondes, '' si absente, null si invalide */
function sectionValue() {
  const start = parseTime(cutStart.value);
  const end = parseTime(cutEnd.value);
  if (Number.isNaN(start) || Number.isNaN(end)) return null;
  if (start === null && end === null) return '';
  const s = start ?? 0;
  if (end !== null && end <= s) return null;
  return `${s}-${end ?? 'inf'}`;
}
function sectionLabel(section) {
  const fmt = (v) => {
    const n = Math.round(Number(v));
    const h = Math.floor(n / 3600), m = Math.floor((n % 3600) / 60), sec = n % 60;
    return `${h ? `${h}:${String(m).padStart(2, '0')}` : m}:${String(sec).padStart(2, '0')}`;
  };
  const [start, end] = section.split('-');
  return `${fmt(start)} → ${end === 'inf' ? 'fin' : fmt(end)}`;
}
const urlCount = computed(() => parseUrls().length);
const hasFinished = computed(() => jobs.value.some((j) => ['done', 'error', 'canceled'].includes(j.status)));

function parseUrls() {
  return urlsText.value.split(/[\n\s]+/).map((s) => s.trim()).filter(Boolean);
}
function persistJobs() {
  localStorage.setItem('fs-jobs', JSON.stringify(jobs.value.slice(0, 100)));
}
function findJob(id) { return jobs.value.find((j) => j.id === id); }

function formatLabel(f) {
  if (!f) return '';
  if (f.startsWith('a-')) return `🎵 ${f.slice(2).toUpperCase()}`;
  const v = f.match(/^v-([\w]+)-([\w]+)-(audio|mute)$/);
  if (v) return `🎬 ${v[1]} ${v[2].toUpperCase()}${v[3] === 'mute' ? ' · muet' : ''}`;
  const s = f.match(/^s-([\w]+)-([\w]+)-([\w]+)$/);
  if (s) return `🎬+🎵 ${s[1]} ${s[2].toUpperCase()} + ${s[3].toUpperCase()}`;
  return f;
}
function fmtDuration(s) {
  if (s == null) return '';
  const m = Math.floor(s / 60), sec = Math.round(s % 60);
  return `${m}:${String(sec).padStart(2, '0')}`;
}
function jobManifest(job) { return job.manifest || null; }
function itemPosition(job) {
  const m = jobManifest(job);
  if (m && job.item_index) {
    const pos = m.findIndex((e) => e.i === job.item_index);
    if (pos >= 0) return { pos: pos + 1, total: m.length };
  }
  return { pos: job.item_index, total: job.item_count };
}
function entryState(job, entryIndex) {
  if (job.status === 'done') return '✅';
  if (job.status !== 'running' || !job.item_index) return '⏳';
  if (entryIndex < job.item_index) return '✅';
  if (entryIndex === job.item_index) return '⬇️';
  return '⏳';
}
function toggleQueue(id) {
  const s = new Set(expandedQueue.value);
  s.has(id) ? s.delete(id) : s.add(id);
  expandedQueue.value = s;
}
function baseName(p) {
  return (p || '').split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '') || '';
}
function fileName(p) { return (p || '').split(/[\\/]/).pop() || ''; }

/* ===== Setup outils ===== */
async function boot() {
  const st = await invoke('tools_status');
  if (!destDir.value) {
    destDir.value = await invoke('default_download_dir');
    localStorage.setItem('fs-dest', destDir.value);
  }
  if (st.ytdlp && st.ffmpeg && st.deno) {
    toolsReady.value = true;
    return;
  }
  toolsReady.value = false;
  try {
    await invoke('setup_tools');
    toolsReady.value = true;
  } catch (err) {
    setupError.value = String(err);
  }
}

async function chooseDest() {
  const dir = await open({ directory: true, defaultPath: destDir.value });
  if (dir) {
    destDir.value = dir;
    localStorage.setItem('fs-dest', dir);
  }
}

/* ===== Jobs ===== */
function startInvoke(job) {
  invoke('start_job', {
    id: job.id, url: job.url, format: job.format,
    dest: job.dest, playlist: Boolean(job.playlist), items: job.items,
    subsMode: job.subsMode || null, subsLangs: job.subsLangs || null, section: job.section || null
  }).catch((err) => {
    const j = findJob(job.id);
    if (j) { j.status = 'error'; j.error = String(err); persistJobs(); }
  });
}

function launch(url, playlist = false, items = null, manifest = null) {
  const section = sectionValue();
  if (section === null) {
    submitError.value = 'découpe : horodatages invalides (ex. 1:20 → 3:45)';
    return;
  }
  const useSubs = mode.value !== 'audio' && subsMode.value !== 'none';
  const job = {
    id: crypto.randomUUID(),
    url, format: format.value, playlist, items, manifest,
    dest: destDir.value,
    subsMode: useSubs ? subsMode.value : null,
    subsLangs: useSubs ? (subsLangs.value.trim() || 'fr,en') : null,
    section: section || null,
    status: 'running', progress: 0, speed: '', eta: '',
    title: null, upload_date: null, files: [], error: null,
    item_index: null, item_count: null, item_title: null
  };
  jobs.value.unshift(job);
  persistJobs();
  startInvoke(job);
}

/* reprise d'un job interrompu par une fermeture de l'app : même id → même
 * dossier temporaire .fs-<id>, yt-dlp reprend les fichiers déjà téléchargés */
function resumeJob(job) {
  job.status = 'running';
  job.error = null;
  job.speed = '';
  job.eta = '';
  if (!job.dest) job.dest = destDir.value;
  startInvoke(job);
}

async function submit() {
  submitError.value = '';
  const urls = parseUrls();
  if (!urls.length) return;
  if (sectionValue() === null) {
    submitError.value = 'découpe : horodatages invalides (ex. 1:20 → 3:45)';
    return;
  }
  urlsText.value = '';
  for (const u of urls) launch(u);
}

async function analyze() {
  submitError.value = '';
  const urls = parseUrls();
  if (urls.length !== 1) {
    submitError.value = 'collez une seule URL de playlist pour choisir les titres';
    return;
  }
  // mix radio YouTube (list=RD…) : généré à la volée par YouTube, qui bloque
  // sa lecture par les outils tiers — impossible à lister ou télécharger en playlist
  if (/[?&]list=RD/.test(urls[0])) {
    submitError.value = 'Ce lien est un Mix radio YouTube : généré à la volée, il ne peut pas être listé ni téléchargé en playlist (blocage YouTube). « Télécharger » récupérera la vidéo seule. Astuce : ouvrez le mix sur YouTube, enregistrez-le comme vraie playlist, puis collez ce nouveau lien.';
    return;
  }
  inspecting.value = true;
  try {
    const info = await invoke('inspect_url', { url: urls[0] });
    if (!info.is_playlist || !info.entries.length) {
      submitError.value = "ce lien n'est pas une playlist — téléchargement direct possible";
      return;
    }
    picker.value = {
      url: urls[0], title: info.title, entries: info.entries,
      selected: new Set(info.entries.map((e) => e.index))
    };
  } catch (err) {
    submitError.value = String(err);
  } finally {
    inspecting.value = false;
  }
}
function toggleEntry(idx) {
  const s = new Set(picker.value.selected);
  s.has(idx) ? s.delete(idx) : s.add(idx);
  picker.value = { ...picker.value, selected: s };
}
function selectAll(on) {
  picker.value = { ...picker.value, selected: new Set(on ? picker.value.entries.map((e) => e.index) : []) };
}
function confirmPicker() {
  const items = [...picker.value.selected];
  if (!items.length) return;
  const all = items.length === picker.value.entries.length;
  const manifest = picker.value.entries
    .filter((e) => all || items.includes(e.index))
    .map((e) => ({ i: e.index, t: e.title }));
  const url = picker.value.url;
  picker.value = null;
  urlsText.value = '';
  launch(url, true, all ? null : items, manifest);
}

async function cancel(job) {
  await invoke('cancel_job', { id: job.id }).catch(() => {});
  job.status = 'canceled';
  persistJobs();
}
function removeJob(job) {
  jobs.value = jobs.value.filter((j) => j.id !== job.id);
  persistJobs();
}
function clearDone() {
  jobs.value = jobs.value.filter((j) => !['done', 'error', 'canceled'].includes(j.status));
  persistJobs();
}
async function openFile(job) {
  if (job.files?.length === 1) await openPath(job.files[0]).catch(() => {});
  else if (job.files?.length) await revealItemInDir(job.files[0]).catch(() => {});
}
async function showInFolder(job) {
  if (job.files?.length) await revealItemInDir(job.files[0]).catch(() => {});
}
async function renameManual(job) {
  if (job.files?.length !== 1) return;
  const name = window.prompt('Nouveau nom du fichier (sans extension) :', baseName(job.files[0]));
  if (name === null || !name.trim()) return;
  try {
    job.files = [await invoke('rename_file', { path: job.files[0], newBase: name.trim() })];
    persistJobs();
  } catch (err) {
    window.alert(`Renommage impossible : ${err}`);
  }
}
async function renameAi(job) {
  if (job.files?.length !== 1) return;
  busyRename.value = new Set(busyRename.value).add(job.id);
  try {
    job.files = [await invoke('ai_rename', {
      path: job.files[0], title: job.title || '', uploadDate: job.upload_date || '',
      format: job.format, url: job.url
    })];
    persistJobs();
  } catch (err) {
    window.alert(`Renommage IA impossible : ${err}`);
  } finally {
    const next = new Set(busyRename.value);
    next.delete(job.id);
    busyRename.value = next;
  }
}

onMounted(async () => {
  unlisteners.push(await listen('setup-progress', (e) => {
    setupStep.value = e.payload.step;
    setupProgress.value = e.payload.progress;
  }));
  unlisteners.push(await listen('job-progress', (e) => {
    const j = findJob(e.payload.id);
    if (!j) return;
    Object.assign(j, {
      progress: e.payload.progress, speed: e.payload.speed, eta: e.payload.eta,
      item_index: e.payload.item_index, item_count: e.payload.item_count, item_title: e.payload.item_title
    });
  }));
  unlisteners.push(await listen('job-meta', (e) => {
    const j = findJob(e.payload.id);
    if (j) { j.title = e.payload.title; j.upload_date = e.payload.upload_date; }
  }));
  unlisteners.push(await listen('job-done', (e) => {
    const j = findJob(e.payload.id);
    if (!j) return;
    if (j.status === 'canceled') { persistJobs(); return; }
    j.status = e.payload.ok ? 'done' : 'error';
    j.error = e.payload.error;
    j.files = e.payload.files;
    j.progress = e.payload.ok ? 100 : j.progress;
    persistJobs();
  }));
  await boot();
  if (toolsReady.value === true) {
    // mise à jour yt-dlp AVANT la reprise (l'exe ne doit pas être en cours d'usage)
    try {
      const r = await invoke('update_ytdlp');
      if (r.updated) {
        ytdlpNote.value = `yt-dlp mis à jour${r.version ? ` (${r.version})` : ''} ✓`;
        setTimeout(() => { ytdlpNote.value = ''; }, 12_000);
      }
    } catch { /* hors-ligne ou déjà à jour : silencieux */ }
    // reprise des téléchargements interrompus par une fermeture de l'app
    jobs.value.filter((j) => ['running', 'pending'].includes(j.status)).forEach(resumeJob);
  }
});
onBeforeUnmount(() => unlisteners.forEach((u) => u()));
</script>

<template>
  <!-- ===== Installation des outils ===== -->
  <div v-if="toolsReady !== true" class="auth-wrap">
    <div class="card">
      <div class="logo3d big">
        <div class="cube">
          <div class="face f1"></div><div class="face f2"></div><div class="face f3"></div>
          <div class="face f4"></div><div class="face f5"></div><div class="face f6"></div>
          <div class="arrow">⇣</div>
        </div>
      </div>
      <h1>ForgeScoop</h1>
      <p class="sub">Téléchargeur vidéo & audio pour Windows</p>
      <template v-if="toolsReady === false && !setupError">
        <p class="hint" style="text-align:center">
          Première installation : téléchargement de yt-dlp et FFmpeg…<br />
          <strong>{{ setupStep }}</strong>
        </p>
        <div class="progress-wrap">
          <div class="progress-bar"><div class="progress-fill" :style="{ width: `${setupProgress}%` }"></div></div>
          <span class="progress-meta">{{ setupProgress.toFixed(0) }}%</span>
        </div>
      </template>
      <p v-else-if="setupError" class="error-msg">{{ setupError }}
        <button class="small" style="margin-top:10px" @click="setupError = ''; boot()">Réessayer</button>
      </p>
      <p v-else class="hint" style="text-align:center">Vérification…</p>
    </div>
  </div>

  <!-- ===== Application ===== -->
  <template v-else>
    <div class="header">
      <div class="logo3d">
        <div class="cube">
          <div class="face f1"></div><div class="face f2"></div><div class="face f3"></div>
          <div class="face f4"></div><div class="face f5"></div><div class="face f6"></div>
          <div class="arrow">⇣</div>
        </div>
      </div>
      <div>
        <h1>ForgeScoop</h1>
        <div class="sub">Windows · v1.2.0<template v-if="ytdlpNote"> · {{ ytdlpNote }}</template></div>
      </div>
      <div class="spacer"></div>
      <button class="ghost small" @click="settingsOpen = true">⚙️ Paramètres</button>
    </div>

    <div class="card">
      <textarea v-model="urlsText" rows="3"
        placeholder="Collez une ou plusieurs URLs (YouTube, TikTok, Instagram, Facebook, X…) — une par ligne"></textarea>
      <div class="form-row">
        <select v-model="mode">
          <option v-for="m in MODES" :key="m.value" :value="m.value">{{ m.label }}</option>
        </select>
        <select v-if="mode !== 'audio'" v-model="quality">
          <option v-for="qu in QUALITIES" :key="qu.value" :value="qu.value">{{ qu.label }}</option>
        </select>
        <select v-if="mode !== 'audio'" v-model="container">
          <option v-for="ct in CONTAINERS" :key="ct.value" :value="ct.value">{{ ct.label }}</option>
        </select>
        <select v-if="mode !== 'video'" v-model="audioFormat">
          <option v-for="af in AUDIO_FORMATS" :key="af.value" :value="af.value">{{ af.label }}</option>
        </select>
        <label v-if="mode === 'video'" class="check">
          <input v-model="withSound" type="checkbox" />
          Avec le son
        </label>
      </div>
      <div class="form-row">
        <select v-if="mode !== 'audio'" v-model="subsMode" title="Sous-titres">
          <option value="none">Sans sous-titres</option>
          <option value="srt">💬 Sous-titres SRT (fichier)</option>
          <option value="vtt">💬 Sous-titres VTT (fichier)</option>
          <option value="embed" :disabled="!embedOk">💬 Incrustés dans la vidéo</option>
        </select>
        <input v-if="mode !== 'audio' && subsMode !== 'none'" v-model="subsLangs" class="short" type="text"
          placeholder="langues : fr,en" title="Codes langues séparés par des virgules (* = toutes)" />
        <span title="Découpe : ne télécharger qu'un extrait">✂️</span>
        <input v-model="cutStart" class="short" type="text" placeholder="Début (1:20)" title="Laisser vide = depuis le début" />
        <input v-model="cutEnd" class="short" type="text" placeholder="Fin (3:45)" title="Laisser vide = jusqu'à la fin" />
      </div>
      <div class="form-row">
        <button class="small ghost" style="min-width:0; max-width:100%; overflow:hidden; text-overflow:ellipsis" :title="destDir" @click="chooseDest">
          📁 {{ destDir }}
        </button>
        <button :disabled="inspecting || urlCount !== 1" @click="analyze">
          {{ inspecting ? '🔍 Analyse…' : '📃 Choisir dans la playlist' }}
        </button>
        <div class="spacer"></div>
        <button class="primary" :disabled="!urlCount" @click="submit()">
          ⬇️ Télécharger{{ urlCount > 1 ? ` (${urlCount})` : '' }}
        </button>
      </div>
      <p v-if="submitError" class="error-msg">{{ submitError }}</p>
      <p class="hint">1000+ sites supportés · fichiers enregistrés directement dans le dossier choisi ·
        les téléchargements interrompus reprennent au prochain lancement.</p>
    </div>

    <div class="jobs-head">
      <h2>Téléchargements</h2>
      <div class="spacer"></div>
      <button v-if="hasFinished" class="ghost small" @click="clearDone">Nettoyer les terminés</button>
    </div>

    <p v-if="!jobs.length" class="empty">Aucun téléchargement pour l'instant.</p>

    <div class="jobs-list" :class="`layout-${layout}`">
      <div v-for="job in jobs" :key="job.id" class="job">
        <div class="job-top">
          <div style="flex:1; min-width:0">
            <div class="job-title">{{ job.title || job.url }}</div>
            <div v-if="job.title" class="job-url">{{ job.url }}</div>
          </div>
          <span class="badge format-badge">{{ formatLabel(job.format) }}</span>
          <span v-if="job.section" class="badge format-badge">✂️ {{ sectionLabel(job.section) }}</span>
          <span v-if="job.subsMode" class="badge format-badge">💬 {{ job.subsMode }}</span>
          <span class="badge" :class="job.status">{{ STATUS_LABELS[job.status] || job.status }}</span>
        </div>

        <div v-if="job.status === 'running'" class="progress-wrap">
          <div class="progress-bar">
            <div class="progress-fill" :style="{ width: `${Math.min(100, job.progress || 0)}%` }"></div>
          </div>
          <span class="progress-meta">
            {{ (job.progress || 0).toFixed(1) }}%
            <template v-if="job.speed"> · {{ job.speed }}</template>
            <template v-if="job.eta"> · {{ job.eta }}</template>
          </span>
        </div>
        <p v-if="job.status === 'running' && job.item_index && (job.item_count > 1 || jobManifest(job))" class="job-filename">
          ⬇️ Élément {{ itemPosition(job).pos }}/{{ itemPosition(job).total }}<template v-if="job.item_title"> · {{ job.item_title }}</template>
        </p>

        <template v-if="jobManifest(job)">
          <button class="small ghost" style="margin-top: 10px" @click="toggleQueue(job.id)">
            📃 File de la playlist ({{ jobManifest(job).length }}) {{ expandedQueue.has(job.id) ? '▲' : '▼' }}
          </button>
          <div v-if="expandedQueue.has(job.id)" class="queue-list">
            <div v-for="e in jobManifest(job)" :key="e.i" class="entry" style="cursor: default">
              <span>{{ entryState(job, e.i) }}</span>
              <span class="n">{{ e.i }}</span>
              <span class="t">{{ e.t }}</span>
            </div>
          </div>
        </template>

        <p v-if="job.error" class="job-error">{{ job.error }}</p>
        <p v-if="job.status === 'done' && job.files?.length" class="job-filename">
          📄 {{ job.files.length === 1 ? fileName(job.files[0]) : `${job.files.length} fichiers` }}
        </p>

        <div class="job-actions">
          <template v-if="job.status === 'done' && job.files?.length">
            <button class="primary small" @click="openFile(job)">▶️ Ouvrir</button>
            <button class="small" @click="showInFolder(job)">📁 Dossier</button>
            <template v-if="job.files.length === 1">
              <button class="small" @click="renameManual(job)">✏️ Renommer</button>
              <button class="small" :disabled="busyRename.has(job.id)" @click="renameAi(job)">
                {{ busyRename.has(job.id) ? '🤖 …' : '🤖 Renommer IA' }}
              </button>
            </template>
          </template>
          <button v-if="job.status === 'running'" class="small danger" @click="cancel(job)">Annuler</button>
          <button v-else class="small danger" @click="removeJob(job)">Retirer de la liste</button>
        </div>
      </div>
    </div>

    <!-- ===== Modal paramètres ===== -->
    <div v-if="settingsOpen" class="modal-backdrop" @click.self="settingsOpen = false">
      <div class="modal">
        <div class="modal-head">
          <h3>⚙️ Paramètres</h3>
          <button class="icon ghost" @click="settingsOpen = false">✕</button>
        </div>
        <div class="modal-body">
          <div class="admin-section">
            <h4>Dossier de téléchargement</h4>
            <div class="row-item" style="cursor: pointer" @click="chooseDest">
              <span class="grow">📁 {{ destDir }}</span>
              <span class="pill">changer</span>
            </div>
          </div>
          <div class="admin-section">
            <h4>Thème de couleurs</h4>
            <div class="theme-grid">
              <button v-for="t in THEMES" :key="t.id" class="theme-card" :class="{ active: theme === t.id }" @click="setTheme(t.id)">
                <span class="theme-dot" :style="{ background: t.color }"></span>
                {{ t.label }}
              </button>
            </div>
          </div>
          <div class="admin-section">
            <h4>Couleur personnalisée</h4>
            <div class="row-item" style="cursor: pointer">
              <input type="color" :value="customColor" style="width: 42px; height: 32px; padding: 2px; cursor: pointer" @input="setCustomColor" />
              <span class="grow">Choisissez n'importe quelle couleur — le thème s'adapte</span>
              <span v-if="theme === 'custom'" class="pill free">actif</span>
            </div>
          </div>
          <div class="admin-section">
            <h4>Disposition des téléchargements</h4>
            <div v-for="l in LAYOUTS" :key="l.id" class="row-item" style="cursor: pointer" @click="setLayout(l.id)">
              <input type="radio" name="layout" :checked="layout === l.id" style="accent-color: var(--accent); width: auto" />
              <span class="grow"><strong>{{ l.label }}</strong> — <span class="meta">{{ l.desc }}</span></span>
            </div>
          </div>
          <div class="admin-section">
            <h4>Informations</h4>
            <div class="row-item" style="cursor: pointer" @click="settingsOpen = false; aboutOpen = true">
              <span class="grow">ℹ️ À propos & compatibilité</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ===== Modal à propos ===== -->
    <div v-if="aboutOpen" class="modal-backdrop" @click.self="aboutOpen = false">
      <div class="modal">
        <div class="modal-head">
          <h3>ℹ️ À propos & compatibilité</h3>
          <button class="icon ghost" @click="aboutOpen = false">✕</button>
        </div>
        <div class="modal-body legal">
          <div class="admin-section">
            <h4>ForgeScoop pour Windows</h4>
            <p>Application personnelle et non commerciale (Windows 10/11).
              Moteur : <a href="https://github.com/yt-dlp/yt-dlp" target="_blank" rel="noopener">yt-dlp</a> + FFmpeg,
              téléchargés automatiquement au premier lancement. Les fichiers sont enregistrés localement,
              rien n'est envoyé sur un serveur (le renommage IA transmet uniquement le titre de la vidéo à un service d'IA tiers).</p>
          </div>
          <div class="admin-section">
            <h4>Responsabilité d'usage</h4>
            <p><strong>Vous êtes seul responsable des contenus téléchargés</strong> : ne téléchargez que vos propres contenus,
              des contenus libres de droits, ou dans le cadre de la copie privée autorisée par votre législation.</p>
          </div>
          <div class="admin-section">
            <h4>Plateformes & formats</h4>
            <p>YouTube, TikTok, Instagram, Facebook, X, Twitch, Vimeo, SoundCloud… 1000+ sites.<br />
              <strong>Vidéo :</strong> MP4, MKV, WebM, MOV, AVI, WMV, FLV — 144p à 8K, avec ou sans audio.<br />
              <strong>Audio :</strong> MP3, M4A, Opus, FLAC, WAV. <strong>Mixte :</strong> vidéo + audio séparés.<br />
              <strong>Sous-titres :</strong> fichiers SRT/VTT (auto inclus) ou incrustés (MP4/MKV/WebM).<br />
              <strong>Découpe :</strong> extrait seul (début → fin), coupe aux images clés.<br />
              <strong>Moteur :</strong> yt-dlp mis à jour automatiquement à chaque lancement.</p>
          </div>
        </div>
      </div>
    </div>

    <footer class="footer">
      <a @click="aboutOpen = true">À propos & compatibilité</a> · ForgeScoop pour Windows v1.2.0
    </footer>
  </template>
</template>
