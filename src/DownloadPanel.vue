<script setup>
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const emit = defineEmits(['submit-download']);

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
const picker = ref(null);

const format = computed(() => {
  if (mode.value === 'audio') return `a-${audioFormat.value}`;
  if (mode.value === 'split') return `s-${quality.value}-${container.value}-${audioFormat.value}`;
  return `v-${quality.value}-${container.value}-${withSound.value ? 'audio' : 'mute'}`;
});

const embedOk = computed(() => mode.value !== 'audio' && ['mp4', 'mkv', 'webm'].includes(container.value));
watch(embedOk, (ok) => { if (!ok && subsMode.value === 'embed') subsMode.value = 'srt'; });

function parseTime(raw) {
  const s = String(raw ?? '').trim();
  if (!s) return null;
  if (!/^(\d{1,3}:)?(\d{1,2}:)?\d{1,4}(\.\d{1,3})?$/.test(s)) return NaN;
  const parts = s.split(':').map(Number);
  if (parts.slice(1).some((n) => n >= 60)) return NaN;
  return parts.reduce((acc, n) => acc * 60 + n, 0);
}
function sectionValue() {
  const start = parseTime(cutStart.value);
  const end = parseTime(cutEnd.value);
  if (Number.isNaN(start) || Number.isNaN(end)) return null;
  if (start === null && end === null) return '';
  const s = start ?? 0;
  if (end !== null && end <= s) return null;
  return `${s}-${end ?? 'inf'}`;
}

const urlCount = computed(() => parseUrls().length);
function parseUrls() {
  return urlsText.value.split(/[\n\s]+/).map((s) => s.trim()).filter(Boolean);
}
function fmtDuration(s) {
  if (s == null) return '';
  const m = Math.floor(s / 60), sec = Math.round(s % 60);
  return `${m}:${String(sec).padStart(2, '0')}`;
}

function descriptor(url, playlist, items, manifest, section) {
  const useSubs = mode.value !== 'audio' && subsMode.value !== 'none';
  return {
    url, format: format.value, playlist, items, manifest,
    subsMode: useSubs ? subsMode.value : null,
    subsLangs: useSubs ? (subsLangs.value.trim() || 'fr,en') : null,
    section: section || null
  };
}

async function submit() {
  submitError.value = '';
  const urls = parseUrls();
  if (!urls.length) return;
  const section = sectionValue();
  if (section === null) {
    submitError.value = 'découpe : horodatages invalides (ex. 1:20 → 3:45)';
    return;
  }
  urlsText.value = '';
  for (const u of urls) emit('submit-download', descriptor(u, false, null, null, section));
}

async function analyze() {
  submitError.value = '';
  const urls = parseUrls();
  if (urls.length !== 1) {
    submitError.value = 'collez une seule URL de playlist pour choisir les titres';
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
  const section = sectionValue();
  if (section === null) {
    submitError.value = 'découpe : horodatages invalides (ex. 1:20 → 3:45)';
    return;
  }
  const all = items.length === picker.value.entries.length;
  const manifest = picker.value.entries
    .filter((e) => all || items.includes(e.index))
    .map((e) => ({ i: e.index, t: e.title }));
  const url = picker.value.url;
  picker.value = null;
  urlsText.value = '';
  emit('submit-download', descriptor(url, true, all ? null : items, manifest, section));
}
</script>

<template>
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

  <!-- ===== Modal sélection playlist ===== -->
  <div v-if="picker" class="modal-backdrop" @click.self="picker = null">
    <div class="modal">
      <div class="modal-head">
        <h3>📃 {{ picker.title }}</h3>
        <span class="pill">{{ picker.selected.size }}/{{ picker.entries.length }}</span>
        <button class="icon ghost" @click="picker = null">✕</button>
      </div>
      <div class="modal-body">
        <label v-for="e in picker.entries" :key="e.index" class="entry">
          <input type="checkbox" :checked="picker.selected.has(e.index)" @change="toggleEntry(e.index)" />
          <span class="n">{{ e.index }}</span>
          <span class="t">{{ e.title }}</span>
          <span class="d">{{ fmtDuration(e.duration) }}</span>
        </label>
      </div>
      <div class="modal-foot">
        <button class="small" @click="selectAll(true)">Tout sélectionner</button>
        <button class="small ghost" @click="selectAll(false)">Tout désélectionner</button>
        <div class="spacer"></div>
        <button class="primary" :disabled="!picker.selected.size" @click="confirmPicker">
          ⬇️ Télécharger ({{ picker.selected.size }})
        </button>
      </div>
    </div>
  </div>
</template>
