<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ArrowLeft, Check, ChevronLeft, ChevronRight, Download, FastForward, Film, Flag, Heart, Maximize, Pause, Play, Scissors, SkipForward, Volume2, VolumeX, X } from 'lucide-vue-next'
import { command, demoEnabled, isDesktop, mediaUrl } from '../api'
import { clampClip, formatSize, formatTime, isEditableTarget } from '../domain/library'
import type { Environment, Recording } from '../types'

const props = defineProps<{ recording: Recording; environment: Environment | null; continuousPlay: boolean; nextAvailable: boolean; busy: boolean; autoplay: boolean; shortcutsEnabled: boolean }>()
const emit = defineEmits<{ close: []; favorite: []; next: []; 'toggle-continuous': []; 'export-clip': [start: number, end: number, precise: boolean]; 'export-events': [before: number, after: number] }>()
const video = ref<HTMLVideoElement | null>(null), stage = ref<HTMLElement | null>(null)
const playing = ref(false), current = ref(0), actualDuration = ref(0), rate = ref(1), muted = ref(false), error = ref('')
const clipOpen = ref(false), clipStart = ref(0), clipEnd = ref(0), precise = ref(true), before = ref(5), after = ref(3)
const hover = ref<{ time: number; percent: number; image: string } | null>(null)
let thumbnailTimer: ReturnType<typeof setTimeout> | undefined, thumbnailGeneration = 0
const duration = computed(() => actualDuration.value || props.recording.duration || 0)
const source = computed(() => mediaUrl(props.recording.favoritePath || props.recording.path))
const progress = computed(() => duration.value ? current.value / duration.value * 100 : 0)
const killEvents = computed(() => props.recording.events.filter(e => e.kind === 'kill'))
const clip = computed(() => clampClip(clipStart.value, clipEnd.value, duration.value))
const canExport = computed(() => isDesktop && props.environment?.ffmpegAvailable && !props.busy)
const eventRangeValid = computed(() => Number.isFinite(before.value) && Number.isFinite(after.value) && before.value >= 0 && after.value > 0 && before.value <= 60 && after.value <= 60)
const date = computed(() => new Date(props.recording.createdAt * 1000).toLocaleString('zh-CN', { month: 'long', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZone: demoEnabled ? 'Asia/Shanghai' : undefined }))

watch(() => props.recording.id, async () => {
  current.value = 0; playing.value = false; actualDuration.value = 0; error.value = ''; clipOpen.value = false; hover.value = null; clipStart.value = 0; clipEnd.value = props.recording.duration
  thumbnailGeneration++
  await nextTick(); if (video.value) video.value.load()
}, { immediate: true })
function loaded() { if (!video.value) return; actualDuration.value = Number.isFinite(video.value.duration) ? video.value.duration : 0; clipEnd.value = duration.value; video.value.playbackRate = rate.value; if (props.autoplay) void video.value.play().catch(() => { /* Autoplay may require a user gesture in browser preview. */ }) }
function seek(time: number) { if (video.value) { video.value.currentTime = Math.max(0, Math.min(duration.value || Infinity, time)); current.value = video.value.currentTime } }
function frame(direction: number) { if (!video.value) return; video.value.pause(); seek(current.value + direction / (props.recording.fps || 30)) }
async function togglePlay() { if (!video.value) return; if (!video.value.paused) video.value.pause(); else { try { await video.value.play() } catch { error.value = '无法播放此视频，请确认文件可用且编码受支持。' } } }
function changeRate(event: Event) { rate.value = Number((event.target as HTMLSelectElement).value); if (video.value) video.value.playbackRate = rate.value }
function toggleMute() { muted.value = !muted.value; if (video.value) video.value.muted = muted.value }
async function fullscreen() { if (!stage.value) return; if (document.fullscreenElement) await document.exitFullscreen(); else await stage.value.requestFullscreen() }
function ended() { playing.value = false; if (props.continuousPlay && props.nextAvailable) emit('next') }
function hoverTimeline(event: MouseEvent) {
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect(), percent = Math.max(0, Math.min(100, (event.clientX - bounds.left) / bounds.width * 100)), time = percent / 100 * duration.value
  hover.value = { time, percent, image: '' }; clearTimeout(thumbnailTimer)
  const generation = ++thumbnailGeneration
  if (!isDesktop || !props.environment?.ffmpegAvailable) return
  thumbnailTimer = setTimeout(async () => {
    try { const path = await command<string>('get_thumbnail', { id: props.recording.id, time }); if (hover.value && generation === thumbnailGeneration) hover.value.image = mediaUrl(path) } catch { /* A missing preview must not interrupt playback. */ }
  }, 180)
}
function leaveTimeline() { hover.value = null; clearTimeout(thumbnailTimer); thumbnailGeneration++ }
function keydown(event: KeyboardEvent) {
  if (!props.shortcutsEnabled || isEditableTarget(event.target) || event.altKey || event.ctrlKey || event.metaKey) return
  if (event.target instanceof HTMLElement && event.target.closest('button') && [' ', 'Enter'].includes(event.key)) return
  const action: Record<string, () => void> = { ' ': () => { void togglePlay() }, k: () => { void togglePlay() }, ArrowLeft: () => seek(current.value - 5), ArrowRight: () => seek(current.value + 5), j: () => seek(current.value - 10), l: () => seek(current.value + 10), ',': () => frame(-1), '.': () => frame(1), m: toggleMute, f: () => { void fullscreen() }, Escape: () => { if (!document.fullscreenElement) emit('close') } }
  const handler = action[event.key] || action[event.key.toLowerCase()]
  if (handler) { event.preventDefault(); handler() }
}
onMounted(() => window.addEventListener('keydown', keydown))
onBeforeUnmount(() => { window.removeEventListener('keydown', keydown); clearTimeout(thumbnailTimer); thumbnailGeneration++ })
</script>

<template>
  <section class="player-page" aria-label="录像播放器">
    <div class="player-heading">
      <button class="text-button" @click="emit('close')"><ArrowLeft :size="17" /> 返回录像库</button>
      <span v-if="demoEnabled" class="demo-pill">演示视频 · 花卉公共样例 / 模拟事件</span>
      <div class="player-heading-actions"><button class="icon-button" :class="{ 'is-favorite': recording.favoriteMode !== 'none' }" @click="emit('favorite')" :title="recording.favoriteMode === 'none' ? '收藏' : '调整收藏'"><Heart :size="19" :fill="recording.favoriteMode !== 'none' ? 'currentColor' : 'none'" /></button><button class="icon-button" aria-label="关闭播放器" @click="emit('close')"><X :size="19" /></button></div>
    </div>
    <div class="player-layout">
      <div class="player-main">
        <div ref="stage" class="video-stage">
          <video v-if="recording.available || recording.favoritePath" ref="video" :src="source" preload="metadata" playsinline @loadedmetadata="loaded" @timeupdate="current = video?.currentTime || 0" @play="playing = true" @pause="playing = false" @ended="ended" @error="error = '视频加载失败。文件可能已移动，或当前系统不支持该视频编码。'" @click="togglePlay" />
          <div v-else class="video-unavailable"><Film :size="42" /><h3>原始录像已不可用</h3><p>重新连接来源目录，或选择已保存的永久副本。</p></div>
          <div v-if="error" class="video-error"><Film :size="25" /><p>{{ error }}</p></div>
          <button v-if="!playing && !error && recording.available" class="center-play" aria-label="播放录像" @click="togglePlay"><Play :size="28" fill="currentColor" /></button>
          <div class="video-quality">{{ recording.height ? `${recording.height}p` : '分辨率未知' }}<span>{{ recording.fps ? `${Math.round(recording.fps)} FPS` : 'FPS 未知' }}</span></div>
        </div>
        <div class="playback-panel">
          <div class="timeline" @mousemove="hoverTimeline" @mouseleave="leaveTimeline">
            <div class="timeline-track"><div class="timeline-fill" :style="{ width: `${progress}%` }" /></div>
            <input class="timeline-range" aria-label="播放进度" type="range" :min="0" :max="duration || 1" step="0.01" :value="current" @input="seek(Number(($event.target as HTMLInputElement).value))" />
            <button v-for="event in recording.events" :key="event.id" class="timeline-event" :class="event.kind" :style="{ left: `${Math.min(100, Math.max(0, event.time / (duration || 1) * 100))}%` }" :aria-label="`${event.label} ${formatTime(event.time)}`" :title="`${event.label} · ${formatTime(event.time)}`" @click="seek(event.time)" />
            <div v-if="hover" class="timeline-preview" :style="{ left: `${Math.max(8, Math.min(92, hover.percent))}%` }"><img v-if="hover.image" :src="hover.image" alt="此时间点的缩略图" /><span>{{ formatTime(hover.time) }}</span></div>
          </div>
          <div class="playback-controls">
            <button class="icon-button control-play" :aria-label="playing ? '暂停' : '播放'" @click="togglePlay"><Pause v-if="playing" :size="19" fill="currentColor" /><Play v-else :size="19" fill="currentColor" /></button>
            <button class="icon-button" aria-label="上一帧" title="上一帧（,）" @click="frame(-1)"><ChevronLeft :size="19" /></button><button class="icon-button" aria-label="下一帧" title="下一帧（.）" @click="frame(1)"><ChevronRight :size="19" /></button>
            <span class="playback-time">{{ formatTime(current) }} <span>/ {{ formatTime(duration) }}</span></span>
            <div class="controls-spacer" />
            <button class="icon-button" :aria-label="muted ? '取消静音' : '静音'" title="静音（M）" @click="toggleMute"><VolumeX v-if="muted" :size="18" /><Volume2 v-else :size="18" /></button>
            <select class="speed-select" aria-label="播放速度" :value="rate" @change="changeRate"><option v-for="speed in [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3]" :key="speed" :value="speed">{{ speed }}×</option></select>
            <button class="icon-button" :disabled="!nextAvailable" aria-label="下一个录像" title="下一个录像" @click="emit('next')"><SkipForward :size="19" /></button>
            <button class="icon-button" aria-label="全屏" title="全屏（F）" @click="fullscreen"><Maximize :size="18" /></button>
          </div>
        </div>
        <div class="video-detail">
          <div><span class="eyebrow">{{ recording.account || '未知账号' }}</span><h1>{{ recording.title }}</h1><p>{{ date }} <span>·</span> {{ formatSize(recording.size) }} <span>·</span> {{ recording.metadataStatus }}</p></div>
          <button class="primary-button" :class="{ active: clipOpen }" @click="clipOpen = !clipOpen"><Scissors :size="17" /> {{ clipOpen ? '收起裁剪' : '裁剪与导出' }}</button>
        </div>
        <div v-if="clipOpen" class="clip-panel">
          <div class="clip-title"><Scissors :size="18" /><strong>导出精彩片段</strong><span>MP4 格式</span></div>
          <div class="clip-range-fields"><label>开始（秒）<div><input v-model.number="clipStart" type="number" min="0" :max="duration" step="0.1" /><button class="text-button" @click="clipStart = Math.round(current * 10) / 10">设为当前</button></div></label><span class="clip-dash">—</span><label>结束（秒）<div><input v-model.number="clipEnd" type="number" min="0" :max="duration" step="0.1" /><button class="text-button" @click="clipEnd = Math.round(current * 10) / 10">设为当前</button></div></label><div class="clip-summary">{{ clip.valid ? formatTime(clip.end - clip.start) : '请设置有效范围' }}<small>片段时长</small></div></div>
          <div class="clip-footer"><label class="checkbox-label"><input v-model="precise" type="checkbox" /> 精确裁剪（重新编码）</label><button class="primary-button" :disabled="!clip.valid || !canExport" @click="emit('export-clip', clip.start, clip.end, precise)"><Download :size="16" /> 导出片段</button></div>
          <p v-if="!environment?.ffmpegAvailable" class="inline-hint">导出与缩略图需要 FFmpeg。请在设置中指定 ffmpeg.exe。</p>
        </div>
        <div class="shortcut-hint"><span><kbd>Space</kbd> 播放 / 暂停</span><span><kbd>←</kbd><kbd>→</kbd> 跳转 5 秒</span><span><kbd>,</kbd><kbd>.</kbd> 逐帧</span><span><kbd>F</kbd> 全屏</span></div>
      </div>
      <aside class="event-sidebar">
        <div class="event-header"><Flag :size="17" /><h2>事件时间轴</h2><span>{{ recording.events.length }}</span></div>
        <p class="event-subtitle">点击事件，直达精彩瞬间</p>
        <div class="event-list"><button v-for="event in recording.events" :key="event.id" class="event-row" :class="{ current: Math.abs(current - event.time) < 1.5 }" @click="seek(event.time)"><span class="event-symbol" :class="event.kind"><Check v-if="event.kind === 'kill'" :size="15" /><Flag v-else :size="14" /></span><div><strong>{{ event.label }}</strong><small>{{ { kill: '击杀事件', death: '阵亡事件', assist: '助攻事件', round: '回合事件', unknown: '元数据事件' }[event.kind] }}</small></div><time>{{ formatTime(event.time) }}</time></button><div v-if="!recording.events.length" class="event-empty"><Flag :size="28" /><p>暂无可验证的事件</p><small>连接 ACLOS 元数据后，支持的事件将自动出现。</small></div></div>
        <div class="event-export"><h3><Scissors :size="16" /> 按击杀事件截取</h3><p>为每个击杀事件导出独立片段</p><div class="event-padding"><label>前 <input v-model.number="before" type="number" min="0" max="60" step="1" /> 秒</label><label>后 <input v-model.number="after" type="number" min="0" max="60" step="1" /> 秒</label></div><button class="secondary-button" :disabled="!killEvents.length || !canExport || !eventRangeValid" @click="emit('export-events', before, after)"><Download :size="16" /> 导出 {{ killEvents.length }} 个击杀片段</button></div>
        <button class="continuous-toggle" :aria-pressed="continuousPlay" @click="emit('toggle-continuous')"><FastForward :size="17" /><span>连续播放</span><span class="toggle" :class="{ on: continuousPlay }"><i /></span></button>
        <div class="file-detail"><span>文件位置</span><p :title="recording.path">{{ recording.path }}</p><span v-if="recording.favoritePath">已保留永久副本{{ recording.favoriteMode === 'none' ? ' · 当前未收藏' : '' }}</span><span v-else-if="recording.favoriteMode === 'index'">索引收藏 · 依赖原始文件</span></div>
      </aside>
    </div>
  </section>
</template>
