<script setup>
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const props = defineProps({ defaultDest: { type: String, default: '' } });
const emit = defineEmits(['submit-convert']);

const mode = ref('video'); // video | audio
const inputPath = ref('');
const info = ref(null); // { duration, width, height } — aperçu ffprobe
const targetVideo = ref('mp4');
const targetAudio = ref('mp3');
const loudnorm = ref(false);
const destOverride = ref('');

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
const fileName = computed(() => (inputPath.value ? inputPath.value.split(/[\\/]/).pop() : ''));

watch(mode, () => { if (mode.value === 'video') loudnorm.value = false; });

function fmtDuration(s) {
  if (!s) return '';
  const m = Math.floor(s / 60), sec = Math.round(s % 60);
  return `${m}:${String(sec).padStart(2, '0')}`;
}

async function pickFile() {
  const filters = mode.value === 'video'
    ? [{ name: 'Vidéo', extensions: ['mp4', 'mkv', 'webm', 'mov', 'avi', 'wmv', 'flv', 'm4v', 'ts'] }]
    : [{ name: 'Audio ou vidéo', extensions: ['mp3', 'm4a', 'aac', 'opus', 'flac', 'wav', 'ogg', 'mp4', 'mkv', 'webm', 'mov', 'avi', 'wmv', 'flv'] }];
  const path = await open({ multiple: false, filters });
  if (!path) return;
  inputPath.value = path;
  info.value = await invoke('probe_info', { path }).catch(() => null);
}

async function pickDest() {
  const dir = await open({ directory: true, defaultPath: destOverride.value || props.defaultDest });
  if (dir) destOverride.value = dir;
}

function submit() {
  if (!inputPath.value) return;
  const kind = mode.value === 'video' ? 'convert-video' : 'audio';
  emit('submit-convert', { kind, input: inputPath.value, target: target.value, loudnorm: loudnorm.value, dest: destOverride.value || null });
  inputPath.value = '';
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
      <button @click="pickFile">📂 Choisir un fichier</button>
      <span v-if="fileName" class="hint" style="margin:0">{{ fileName }}</span>
    </div>
    <p v-if="info" class="hint">
      {{ fmtDuration(info.duration) }}<template v-if="info.width && info.height"> · {{ info.width }}×{{ info.height }}</template>
    </p>
    <p v-if="mode === 'audio'" class="hint">
      Fichier audio → converti dans le format choisi. Fichier vidéo → la piste audio est extraite (la vidéo est ignorée).
    </p>
    <div class="form-row">
      <button class="small ghost" style="min-width:0; max-width:100%; overflow:hidden; text-overflow:ellipsis" :title="destOverride || defaultDest" @click="pickDest">
        📁 {{ destOverride || defaultDest }}
      </button>
      <div class="spacer"></div>
      <button class="primary" :disabled="!inputPath" @click="submit">🔄 Convertir</button>
    </div>
  </div>
</template>
