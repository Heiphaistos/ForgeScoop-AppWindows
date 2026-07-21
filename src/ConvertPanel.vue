<script setup>
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const props = defineProps({ defaultDest: { type: String, default: '' } });
const emit = defineEmits(['submit-convert']);

const mode = ref('video'); // video | audio
const items = ref([]); // [{ path, selected }]
const info = ref(null); // aperçu ffprobe — seulement si un fichier unique choisi
const targetVideo = ref('mp4');
const targetAudio = ref('mp3');
const loudnorm = ref(false);
const destOverride = ref('');
const pickBusy = ref(false);
const pickError = ref('');

const VIDEO_TARGETS = [
  { value: 'mp4', label: 'MP4 (universel)' },
  { value: 'mkv', label: 'MKV' },
  { value: 'webm', label: 'WebM' },
  { value: 'mov', label: 'MOV (Apple)' },
  { value: 'avi', label: 'AVI' },
  { value: 'wmv', label: 'WMV' },
  { value: 'flv', label: 'FLV' },
  { value: 'gif', label: 'GIF animé' }
];
const AUDIO_TARGETS = [
  { value: 'mp3', label: 'MP3' },
  { value: 'm4a', label: 'M4A (AAC)' },
  { value: 'aac', label: 'AAC' },
  { value: 'opus', label: 'Opus' },
  { value: 'flac', label: 'FLAC (sans perte)' },
  { value: 'wav', label: 'WAV' },
  { value: 'ogg', label: 'OGG (Vorbis)' }
];

const target = computed(() => (mode.value === 'video' ? targetVideo.value : targetAudio.value));
const selectedCount = computed(() => items.value.filter((it) => it.selected).length);

function fileName(p) { return p.split(/[\\/]/).pop(); }

async function setSingle(path) {
  items.value = [{ path, selected: true }];
  info.value = await invoke('probe_info', { path }).catch(() => null);
}

function fmtDuration(s) {
  if (!s) return '';
  const m = Math.floor(s / 60), sec = Math.round(s % 60);
  return `${m}:${String(sec).padStart(2, '0')}`;
}

async function pickFiles() {
  pickError.value = '';
  const filters = mode.value === 'video'
    ? [{ name: 'Vidéo', extensions: ['mp4', 'mkv', 'webm', 'mov', 'avi', 'wmv', 'flv', 'm4v', 'ts'] }]
    : [{ name: 'Audio ou vidéo', extensions: ['mp3', 'm4a', 'aac', 'opus', 'flac', 'wav', 'ogg', 'mp4', 'mkv', 'webm', 'mov', 'avi', 'wmv', 'flv'] }];
  const paths = await open({ multiple: true, filters });
  if (!paths || !paths.length) return;
  if (paths.length === 1) { await setSingle(paths[0]); return; }
  info.value = null;
  items.value = paths.map((path) => ({ path, selected: true }));
}

async function pickFolder() {
  pickError.value = '';
  const dir = await open({ directory: true });
  if (!dir) return;
  pickBusy.value = true;
  try {
    const paths = await invoke('list_media_files', { dir, mode: mode.value });
    if (!paths.length) { pickError.value = 'aucun fichier média trouvé dans ce dossier'; return; }
    info.value = null;
    items.value = paths.map((path) => ({ path, selected: true }));
  } catch (err) {
    pickError.value = String(err);
  } finally {
    pickBusy.value = false;
  }
}

function selectAll(on) { for (const it of items.value) it.selected = on; }

async function pickDest() {
  const dir = await open({ directory: true, defaultPath: destOverride.value || props.defaultDest });
  if (dir) destOverride.value = dir;
}

watch(mode, () => { if (mode.value === 'video') loudnorm.value = false; items.value = []; info.value = null; });

function submit() {
  const todo = items.value.filter((it) => it.selected);
  if (!todo.length) return;
  const kind = mode.value === 'video' ? 'convert-video' : 'audio';
  for (const it of todo) {
    emit('submit-convert', { kind, input: it.path, target: target.value, loudnorm: mode.value === 'audio' && loudnorm.value, dest: destOverride.value || null });
  }
  items.value = [];
  info.value = null;
}
</script>

<template>
  <div class="card">
    <div class="form-row">
      <select v-model="mode">
        <option value="video">🎬 Convertir une vidéo</option>
        <option value="audio">🎵 Convertir / extraire l'audio</option>
      </select>
      <select v-if="mode === 'video'" v-model="targetVideo">
        <option v-for="t in VIDEO_TARGETS" :key="t.value" :value="t.value">{{ t.label }}</option>
      </select>
      <select v-else v-model="targetAudio">
        <option v-for="t in AUDIO_TARGETS" :key="t.value" :value="t.value">{{ t.label }}</option>
      </select>
      <label v-if="mode === 'audio'" class="check">
        <input v-model="loudnorm" type="checkbox" />
        Normaliser le volume
      </label>
    </div>
    <div class="form-row">
      <button @click="pickFiles">📂 Choisir un ou plusieurs fichiers</button>
      <button :disabled="pickBusy" @click="pickFolder">{{ pickBusy ? '…' : '📁 Ou un dossier entier' }}</button>
    </div>
    <p v-if="pickError" class="error-msg">{{ pickError }}</p>

    <template v-if="items.length > 1">
      <div class="form-row" style="margin-top:10px">
        <button class="small" @click="selectAll(true)">Tout sélectionner</button>
        <button class="small ghost" @click="selectAll(false)">Tout désélectionner</button>
        <span class="hint" style="margin:0">{{ selectedCount }}/{{ items.length }}</span>
      </div>
      <div class="queue-list">
        <label v-for="it in items" :key="it.path" class="entry">
          <input type="checkbox" v-model="it.selected" />
          <span class="t">{{ fileName(it.path) }}</span>
        </label>
      </div>
    </template>
    <p v-else-if="items.length === 1" class="hint">
      {{ fileName(items[0].path) }}
      <template v-if="info">· {{ fmtDuration(info.duration) }}<template v-if="info.width && info.height"> · {{ info.width }}×{{ info.height }}</template></template>
    </p>

    <p v-if="mode === 'audio'" class="hint">
      Fichier audio → converti dans le format choisi. Fichier vidéo → la piste audio est extraite (la vidéo est ignorée).
    </p>
    <div class="form-row">
      <button class="small ghost" style="min-width:0; max-width:100%; overflow:hidden; text-overflow:ellipsis" :title="destOverride || defaultDest" @click="pickDest">
        📁 {{ destOverride || defaultDest }}
      </button>
      <div class="spacer"></div>
      <button class="primary" :disabled="!selectedCount" @click="submit">
        {{ selectedCount > 1 ? `🔄 Convertir (${selectedCount})` : '🔄 Convertir' }}
      </button>
    </div>
  </div>
</template>
