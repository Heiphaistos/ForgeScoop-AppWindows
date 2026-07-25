<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
import { isEnabled as autostartEnabled, enable as autostartEnable, disable as autostartDisable } from '@tauri-apps/plugin-autostart';
import DownloadPanel from './DownloadPanel.vue';
import ConvertPanel from './ConvertPanel.vue';
import MuxPanel from './MuxPanel.vue';

const TABS = [
  { id: 'download', label: '⬇️ Télécharger' },
  { id: 'convert', label: '🔄 Convertir' },
  { id: 'mux', label: '🎬+🎵 Fusionner audio/vidéo' }
];
const activeTab = ref('download');

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

/* ===== Notifications natives ===== */
const notifyEnabled = ref(localStorage.getItem('fs-notify') === '1');
async function toggleNotify() {
  if (notifyEnabled.value) {
    notifyEnabled.value = false;
    localStorage.setItem('fs-notify', '0');
    return;
  }
  let granted = await isPermissionGranted();
  if (!granted) granted = (await requestPermission()) === 'granted';
  if (granted) {
    notifyEnabled.value = true;
    localStorage.setItem('fs-notify', '1');
  } else {
    window.alert('Notifications refusées par Windows — vérifiez les paramètres système.');
  }
}
function notifyJobDone(job) {
  if (!notifyEnabled.value) return;
  sendNotification({ title: 'ForgeScoop — traitement terminé', body: job.title || job.url || 'Fichier prêt' });
}

/* ===== Lancement au démarrage de Windows ===== */
const autostart = ref(false);
async function refreshAutostart() {
  try { autostart.value = await autostartEnabled(); } catch { /* plugin indisponible hors build release */ }
}
async function toggleAutostart() {
  try {
    if (autostart.value) await autostartDisable();
    else await autostartEnable();
    autostart.value = !autostart.value;
  } catch (err) {
    window.alert(`Impossible de changer le démarrage automatique : ${err}`);
  }
}

/* ===== Dossier surveillé (conversion automatique) ===== */
const watchCfg = ref({ enabled: false, folder: '', kind: 'convert-video', target: 'mp4', loudnorm: false });
const watchError = ref('');
async function refreshWatchConfig() {
  try { watchCfg.value = await invoke('get_watch_config'); } catch { /* défaut */ }
}
async function chooseWatchFolder() {
  const dir = await open({ directory: true, defaultPath: watchCfg.value.folder || undefined });
  if (dir) watchCfg.value.folder = dir;
}
async function saveWatchConfig() {
  watchError.value = '';
  try {
    await invoke('set_watch_config', { cfg: watchCfg.value });
  } catch (err) {
    watchError.value = String(err);
  }
}

/* ===== Presse-papier surveillé ===== */
const clipboardWatchEnabled = ref(localStorage.getItem('fs-clipboard-watch') === '1');
const clipboardSuggestion = ref(''); // URL détectée, en attente de décision
let clipboardDismissTimer = null;
async function toggleClipboardWatch() {
  clipboardWatchEnabled.value = !clipboardWatchEnabled.value;
  localStorage.setItem('fs-clipboard-watch', clipboardWatchEnabled.value ? '1' : '0');
  if (clipboardWatchEnabled.value) await invoke('start_clipboard_watch');
  else await invoke('stop_clipboard_watch');
}
function acceptClipboardSuggestion() {
  const url = clipboardSuggestion.value;
  clipboardSuggestion.value = '';
  launchDownload({ url, format: 'v-best-mp4-audio', playlist: false, items: null, manifest: null, subsMode: null, subsLangs: null, section: null, rateLimit: null });
}
function dismissClipboardSuggestion() {
  clipboardSuggestion.value = '';
}

/* ===== Outils (yt-dlp / ffmpeg) ===== */
const toolsReady = ref(null); // null = vérification, false = installation, true = prêt
const setupStep = ref('');
const setupProgress = ref(0);
const setupError = ref('');
const ytdlpNote = ref(''); // note transitoire après la mise à jour auto

/* ===== Téléchargements ===== */
const STATUS_LABELS = { pending: 'en attente', running: 'en cours', done: 'terminé', error: 'erreur', canceled: 'annulé' };

const destDir = ref(localStorage.getItem('fs-dest') || '');
// file persistante : tout l'historique est conservé, les jobs interrompus
// (running/pending au moment de la fermeture) sont repris au démarrage
const jobs = ref(JSON.parse(localStorage.getItem('fs-jobs') || '[]'));
const busyRename = ref(new Set());
const expandedQueue = ref(new Set());
const unlisteners = [];

function sectionLabel(section) {
  const fmt = (v) => {
    const n = Math.round(Number(v));
    const h = Math.floor(n / 3600), m = Math.floor((n % 3600) / 60), sec = n % 60;
    return `${h ? `${h}:${String(m).padStart(2, '0')}` : m}:${String(sec).padStart(2, '0')}`;
  };
  const [start, end] = section.split('-');
  return `${fmt(start)} → ${end === 'inf' ? 'fin' : fmt(end)}`;
}
const hasFinished = computed(() => jobs.value.some((j) => ['done', 'error', 'canceled'].includes(j.status)));

function persistJobs() {
  localStorage.setItem('fs-jobs', JSON.stringify(jobs.value.slice(0, 100)));
}
function findJob(id) { return jobs.value.find((j) => j.id === id); }

function formatLabel(job) {
  const f = job.format;
  if (!f) return '';
  if (job.kind === 'convert-video') return `🔄🎬 ${f.toUpperCase()}`;
  if (job.kind === 'audio') return `🔄🎵 ${f.toUpperCase()}`;
  if (job.kind === 'mux') return `🎬+🎵→🎬 ${f.toUpperCase()}`;
  if (f.startsWith('a-')) return `🎵 ${f.slice(2).toUpperCase()}`;
  const v = f.match(/^v-([\w]+)-([\w]+)-(audio|mute)$/);
  if (v) return `🎬 ${v[1]} ${v[2].toUpperCase()}${v[3] === 'mute' ? ' · muet' : ''}`;
  const s = f.match(/^s-([\w]+)-([\w]+)-([\w]+)$/);
  if (s) return `🎬+🎵 ${s[1]} ${s[2].toUpperCase()} + ${s[3].toUpperCase()}`;
  return f;
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

/* ===== Jobs : téléchargement ===== */
function startInvoke(job) {
  invoke('start_job', {
    id: job.id, url: job.url, format: job.format,
    dest: job.dest, playlist: Boolean(job.playlist), items: job.items,
    subsMode: job.subsMode || null, subsLangs: job.subsLangs || null, section: job.section || null,
    rateLimit: job.rateLimit || null
  }).catch((err) => {
    const j = findJob(job.id);
    if (j) { j.status = 'error'; j.error = String(err); persistJobs(); }
  });
}

/* déclenché par DownloadPanel (@submit-download) — descripteur déjà validé côté panel */
function launchDownload(params) {
  const dest = params.folderName ? `${destDir.value}\\${params.folderName}` : destDir.value;
  const job = {
    id: crypto.randomUUID(),
    kind: 'download',
    url: params.url, format: params.format, playlist: params.playlist, items: params.items, manifest: params.manifest,
    dest,
    subsMode: params.subsMode, subsLangs: params.subsLangs, section: params.section, rateLimit: params.rateLimit,
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

/* ===== Jobs : conversion / extraction / fusion (pas de reprise après
 * fermeture — les jobs de conversion interrompus repassent en erreur).
 * File d'attente locale : convertir un dossier entier ne doit pas lancer
 * autant de ffmpeg simultanés que de fichiers — même principe de
 * concurrence bornée que la file serveur de la version web. */
const MAX_CONCURRENT_CONVERT = 2;
const convertQueue = ref([]); // [{ job, args }] en attente de démarrage

function runningConvertCount() {
  return jobs.value.filter((j) => j.kind && j.kind !== 'download' && j.status === 'running').length;
}
function pumpConvertQueue() {
  while (runningConvertCount() < MAX_CONCURRENT_CONVERT && convertQueue.value.length) {
    const { job, args } = convertQueue.value.shift();
    job.status = 'running';
    persistJobs();
    invoke('start_convert_job', args).catch((err) => {
      const j = findJob(job.id);
      if (j) { j.status = 'error'; j.error = String(err); persistJobs(); }
      pumpConvertQueue();
    });
  }
}

function launchConvert(kind, input, input2, target, opts = {}) {
  const dest = opts.dest || destDir.value;
  const loudnorm = Boolean(opts.loudnorm);
  const label = kind === 'mux' ? `${fileName(input)} + ${fileName(input2)}` : fileName(input);
  const job = {
    id: crypto.randomUUID(),
    kind, url: '', format: target, title: label,
    dest,
    status: 'pending', progress: 0, speed: '', eta: '',
    upload_date: null, files: [], error: null,
    item_index: null, item_count: null, item_title: null
  };
  jobs.value.unshift(job);
  convertQueue.value.push({ job, args: { id: job.id, kind, input, input2, target, dest, loudnorm } });
  persistJobs();
  pumpConvertQueue();
}

async function cancel(job) {
  if (job.status === 'pending') {
    // encore en file, jamais démarré côté Rust : rien à annuler côté process
    convertQueue.value = convertQueue.value.filter((q) => q.job.id !== job.id);
    job.status = 'canceled';
    persistJobs();
    return;
  }
  await invoke('cancel_job', { id: job.id }).catch(() => {});
  job.status = 'canceled';
  persistJobs();
}
function removeJob(job) {
  jobs.value = jobs.value.filter((j) => j.id !== job.id);
  // garde-fou : un job encore en file de conversion ne doit jamais démarrer
  // après avoir été retiré de la liste (ffmpeg orphelin sans job pour le suivre)
  convertQueue.value = convertQueue.value.filter((q) => q.job.id !== job.id);
  persistJobs();
}
function clearDone() {
  jobs.value = jobs.value.filter((j) => !['done', 'error', 'canceled'].includes(j.status));
  persistJobs();
}
async function openFile(job) {
  if (!job.files?.length) return;
  try {
    if (job.files.length === 1) await invoke('open_file', { path: job.files[0] });
    else await invoke('show_in_folder', { path: job.files[0] });
  } catch (err) {
    window.alert(`Ouverture impossible : ${err}`);
  }
}
async function showInFolder(job) {
  if (!job.files?.length) return;
  try {
    await invoke('show_in_folder', { path: job.files[0] });
  } catch (err) {
    window.alert(`Ouverture du dossier impossible : ${err}`);
  }
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
  unlisteners.push(await listen('clipboard-url', (e) => {
    clipboardSuggestion.value = e.payload;
    clearTimeout(clipboardDismissTimer);
    clipboardDismissTimer = setTimeout(() => { clipboardSuggestion.value = ''; }, 20_000);
  }));
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
    notifyJobDone(j);
    pumpConvertQueue();
  }));
  refreshAutostart();
  refreshWatchConfig();
  if (clipboardWatchEnabled.value) invoke('start_clipboard_watch').catch(() => {});
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
    // reprise des téléchargements interrompus par une fermeture de l'app —
    // les conversions/fusions interrompues ne reprennent pas (pas de dossier
    // temporaire ffmpeg à rouvrir), elles repassent simplement en erreur
    const interrupted = jobs.value.filter((j) => ['running', 'pending'].includes(j.status));
    for (const j of interrupted) {
      if (!j.kind || j.kind === 'download') resumeJob(j);
      else { j.status = 'error'; j.error = "traitement interrompu par la fermeture de l'application, relancez-le"; }
    }
    if (interrupted.length) persistJobs();
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
        <div class="sub">Windows · v1.7.0<template v-if="ytdlpNote"> · {{ ytdlpNote }}</template></div>
      </div>
      <div class="spacer"></div>
      <button class="ghost small" @click="settingsOpen = true">⚙️ Paramètres</button>
    </div>

    <div class="form-row" style="margin: 0 0 16px">
      <button class="small ghost" style="min-width:0; max-width:100%; overflow:hidden; text-overflow:ellipsis" :title="destDir" @click="chooseDest">
        📁 {{ destDir }}
      </button>
    </div>

    <div v-if="clipboardSuggestion" class="card" style="margin-bottom: 16px">
      <div class="form-row" style="margin: 0">
        <span class="grow" style="min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap">
          📋 Lien détecté : {{ clipboardSuggestion }}
        </span>
        <button class="primary small" @click="acceptClipboardSuggestion">⬇️ Télécharger</button>
        <button class="small ghost" @click="dismissClipboardSuggestion">Ignorer</button>
      </div>
    </div>

    <div class="tabs">
      <button v-for="t in TABS" :key="t.id" class="tab" :class="{ active: activeTab === t.id }" @click="activeTab = t.id">
        {{ t.label }}
      </button>
    </div>

    <DownloadPanel v-if="activeTab === 'download'" @submit-download="launchDownload" />
    <ConvertPanel v-else-if="activeTab === 'convert'" :default-dest="destDir"
      @submit-convert="(p) => launchConvert(p.kind, p.input, null, p.target, { dest: p.dest, loudnorm: p.loudnorm })" />
    <MuxPanel v-else :default-dest="destDir"
      @submit-mux="(p) => launchConvert('mux', p.input, p.input2, p.target, { dest: p.dest })" />

    <div class="jobs-head">
      <h2>Traitements</h2>
      <div class="spacer"></div>
      <button v-if="hasFinished" class="ghost small" @click="clearDone">Nettoyer les terminés</button>
    </div>

    <p v-if="!jobs.length" class="empty">Aucun traitement pour l'instant.</p>

    <div class="jobs-list" :class="`layout-${layout}`">
      <div v-for="job in jobs" :key="job.id" class="job">
        <div class="job-top">
          <div style="flex:1; min-width:0">
            <div class="job-title">{{ job.title || job.url }}</div>
            <div v-if="job.title && (!job.kind || job.kind === 'download')" class="job-url">{{ job.url }}</div>
          </div>
          <span class="badge format-badge">{{ formatLabel(job) }}</span>
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
          <button v-if="['pending', 'running'].includes(job.status)" class="small danger" @click="cancel(job)">Annuler</button>
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
            <h4>Notifications</h4>
            <div class="row-item" style="cursor: pointer" @click="toggleNotify">
              <input type="checkbox" :checked="notifyEnabled" style="accent-color: var(--accent); width: auto" readonly />
              <span class="grow">🔔 Me notifier quand un traitement est terminé</span>
            </div>
            <div class="row-item" style="cursor: pointer" @click="toggleAutostart">
              <input type="checkbox" :checked="autostart" style="accent-color: var(--accent); width: auto" readonly />
              <span class="grow">🚀 Lancer ForgeScoop au démarrage de Windows</span>
            </div>
            <div class="row-item" style="cursor: pointer" @click="toggleClipboardWatch">
              <input type="checkbox" :checked="clipboardWatchEnabled" style="accent-color: var(--accent); width: auto" readonly />
              <span class="grow">📋 Suggérer un téléchargement quand je copie un lien</span>
            </div>
          </div>
          <div class="admin-section">
            <h4>Dossier surveillé</h4>
            <p class="hint">Déposez un fichier vidéo/audio dans ce dossier : il est converti automatiquement
              (sortie dans le sous-dossier « Converti »). Fonctionne même fenêtre réduite dans la zone de notification.</p>
            <div class="row-item" style="cursor: pointer" @click="watchCfg.enabled = !watchCfg.enabled">
              <input type="checkbox" :checked="watchCfg.enabled" style="accent-color: var(--accent); width: auto" readonly />
              <span class="grow">Activer la surveillance</span>
            </div>
            <div class="row-item" style="cursor: pointer" @click="chooseWatchFolder">
              <span class="grow">📁 {{ watchCfg.folder || 'Choisir un dossier…' }}</span>
              <span class="pill">changer</span>
            </div>
            <div class="form-row">
              <select v-model="watchCfg.kind">
                <option value="convert-video">🔄🎬 Convertir en vidéo</option>
                <option value="audio">🔄🎵 Extraire l'audio</option>
              </select>
              <select v-if="watchCfg.kind === 'convert-video'" v-model="watchCfg.target">
                <option value="mp4">MP4</option><option value="mkv">MKV</option><option value="webm">WebM</option>
                <option value="mov">MOV</option><option value="avi">AVI</option><option value="wmv">WMV</option><option value="flv">FLV</option>
              </select>
              <select v-else v-model="watchCfg.target">
                <option value="mp3">MP3</option><option value="m4a">M4A</option><option value="aac">AAC</option>
                <option value="opus">Opus</option><option value="flac">FLAC</option><option value="wav">WAV</option><option value="ogg">OGG</option>
              </select>
              <label v-if="watchCfg.kind === 'audio'" class="check">
                <input v-model="watchCfg.loudnorm" type="checkbox" /> Normaliser
              </label>
            </div>
            <p v-if="watchError" class="error-msg">{{ watchError }}</p>
            <button class="primary small" style="margin-top: 8px" @click="saveWatchConfig">Enregistrer</button>
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
              <strong>Convertir :</strong> choisissez un fichier vidéo ou audio local et convertissez-le vers n'importe quel format ci-dessus.<br />
              <strong>Fusionner :</strong> remplacez la piste audio d'une vidéo par un autre fichier audio (extraction seule via l'onglet Convertir).<br />
              <strong>Sous-titres :</strong> fichiers SRT/VTT (auto inclus) ou incrustés (MP4/MKV/WebM).<br />
              <strong>Découpe :</strong> extrait seul (début → fin), coupe aux images clés.<br />
              <strong>Moteur :</strong> yt-dlp mis à jour automatiquement à chaque lancement.</p>
          </div>
        </div>
      </div>
    </div>

    <footer class="footer">
      <a @click="aboutOpen = true">À propos & compatibilité</a> · ForgeScoop pour Windows v1.7.0
    </footer>
  </template>
</template>
