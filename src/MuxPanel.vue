<script setup>
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const props = defineProps({ defaultDest: { type: String, default: '' } });
const emit = defineEmits(['submit-mux']);

const videoPath = ref('');
const audioPath = ref('');
const info = ref(null);
const target = ref('mp4');
const destOverride = ref('');

const CONTAINERS = [
  { value: 'mp4', label: 'MP4 (universel)' },
  { value: 'mkv', label: 'MKV' },
  { value: 'webm', label: 'WebM' },
  { value: 'mov', label: 'MOV (Apple)' },
  { value: 'avi', label: 'AVI' },
  { value: 'wmv', label: 'WMV' },
  { value: 'flv', label: 'FLV' }
];

function fileName(p) { return p ? p.split(/[\\/]/).pop() : ''; }
function fmtDuration(s) {
  if (!s) return '';
  const m = Math.floor(s / 60), sec = Math.round(s % 60);
  return `${m}:${String(sec).padStart(2, '0')}`;
}

async function pickVideo() {
  const path = await open({ multiple: false, filters: [{ name: 'Vidéo', extensions: ['mp4', 'mkv', 'webm', 'mov', 'avi', 'wmv', 'flv', 'm4v', 'ts'] }] });
  if (!path) return;
  videoPath.value = path;
  info.value = await invoke('probe_info', { path }).catch(() => null);
}
async function pickAudio() {
  const path = await open({ multiple: false, filters: [{ name: 'Audio', extensions: ['mp3', 'm4a', 'aac', 'opus', 'flac', 'wav', 'ogg'] }] });
  if (path) audioPath.value = path;
}
async function pickDest() {
  const dir = await open({ directory: true, defaultPath: destOverride.value || props.defaultDest });
  if (dir) destOverride.value = dir;
}

function submit() {
  if (!videoPath.value || !audioPath.value) return;
  emit('submit-mux', { input: videoPath.value, input2: audioPath.value, target: target.value, dest: destOverride.value || null });
  videoPath.value = '';
  audioPath.value = '';
  info.value = null;
}
</script>

<template>
  <div class="card">
    <p class="hint" style="margin-top:0">
      Remplace ou ajoute la piste audio d'une vidéo par un autre fichier audio — l'inverse de l'extraction.
    </p>
    <div class="form-row">
      <button @click="pickVideo">🎬 Choisir la vidéo</button>
      <span v-if="videoPath" class="hint" style="margin:0">{{ fileName(videoPath) }}</span>
    </div>
    <p v-if="info" class="hint">
      {{ fmtDuration(info.duration) }}<template v-if="info.width && info.height"> · {{ info.width }}×{{ info.height }}</template>
    </p>
    <div class="form-row">
      <button @click="pickAudio">🎵 Choisir l'audio</button>
      <span v-if="audioPath" class="hint" style="margin:0">{{ fileName(audioPath) }}</span>
    </div>
    <div class="form-row">
      <select v-model="target">
        <option v-for="c in CONTAINERS" :key="c.value" :value="c.value">{{ c.label }}</option>
      </select>
      <button class="small ghost" style="min-width:0; max-width:100%; overflow:hidden; text-overflow:ellipsis" :title="destOverride || defaultDest" @click="pickDest">
        📁 {{ destOverride || defaultDest }}
      </button>
      <div class="spacer"></div>
      <button class="primary" :disabled="!videoPath || !audioPath" @click="submit">🎬+🎵 Fusionner</button>
    </div>
    <p class="hint">La vidéo n'est pas réencodée (copie directe), seul l'audio est traité · durée finale = la plus courte des deux.</p>
  </div>
</template>
