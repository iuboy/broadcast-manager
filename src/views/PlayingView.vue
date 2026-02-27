<template>
  <div class="playing-view">
    <el-card class="now-playing-card">
      <!-- 专辑封面 -->
      <div class="album-cover">
        <el-icon :size="120" class="cover-icon">
          <Headset />
        </el-icon>
        <el-icon v-if="isPlaying" class="playing-indicator">
          <VideoPlay />
        </el-icon>
      </div>

      <!-- 歌曲信息 -->
      <div class="track-info">
        <h2 class="track-title">{{ currentTrackName || '未播放' }}</h2>
        <p class="track-artist">{{ currentTrackArtist || '-' }}</p>
        <p class="track-album">{{ currentTrackAlbum || '-' }}</p>
      </div>

      <!-- 播放进度 -->
      <div class="progress-section">
        <el-slider
          v-model="playProgress"
          :show-tooltip="false"
          :disabled="!hasTrack || !isPlaying"
        />
        <div class="time-info">
          <span>{{ formatTime(currentTime) }}</span>
          <span>{{ formatTime(totalTime) }}</span>
        </div>
      </div>

      <!-- 播放控制 -->
      <div class="playback-controls">
        <el-button-group size="large">
          <el-button @click="previousTrack" :disabled="!isConnected">
            <el-icon><ArrowLeft /></el-icon>
          </el-button>
          <el-button @click="togglePlay" :disabled="!isConnected || !hasTrack" size="large">
            <el-icon>
              <VideoPause v-if="isPlaying" />
              <VideoPlay v-else />
            </el-icon>
          </el-button>
          <el-button @click="nextTrack" :disabled="!isConnected">
            <el-icon><ArrowRight /></el-icon>
          </el-button>
        </el-button-group>
      </div>

      <!-- 音量控制 -->
      <div class="volume-control">
        <el-button
          class="volume-icon-btn"
          :class="{ muted: isMuted }"
          :disabled="!isConnected"
          @click="toggleMute"
          link
        >
          <el-icon class="volume-icon" :size="20">
            <MuteNotification />
          </el-icon>
        </el-button>
        <el-slider
          v-model="volumeValue"
          :min="0"
          :max="100"
          @change="handleVolumeChange"
          :disabled="!isConnected || isMuted"
          style="width: 200px"
        />
        <span class="volume-text">{{ isMuted ? 0 : Math.round(masterVolume * 100) }}%</span>
      </div>
    </el-card>

    <!-- 播放列表 -->
    <el-card class="playlist-card">
      <template #header>
        <div class="card-header">
          <el-icon><List /></el-icon>
          <span>播放列表</span>
        </div>
      </template>
      <el-table
        :data="playlist"
        highlight-current-row
        :current-row-key="currentIndex"
        @row-dblclick="handlePlayTrack"
        style="width: 100%"
        max-height="400"
      >
        <el-table-column type="index" width="50" label="#" />
        <el-table-column prop="title" label="标题" min-width="200">
          <template #default="{ row, $index }">
            <div class="playlist-track-title">
              <el-icon v-if="$index === currentIndex && isPlaying" class="playing-icon">
                <Headset />
              </el-icon>
              <span>{{ row.title }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="artist" label="艺术家" width="150">
          <template #default="{ row }">
            {{ row.artist || '-' }}
          </template>
        </el-table-column>
        <el-table-column prop="album" label="专辑" width="150">
          <template #default="{ row }">
            {{ row.album || '-' }}
          </template>
        </el-table-column>
        <el-table-column prop="duration" label="时长" width="100">
          <template #default="{ row }">
            {{ formatDuration(row.duration) }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="100" fixed="right">
          <template #default="{ row: _row, $index }">
            <el-button
              @click.stop="handlePlayTrackByIndex($index)"
              :disabled="!isConnected"
              size="small"
              :icon="VideoPlay"
              circle
            />
          </template>
        </el-table-column>
      </el-table>
      <el-empty v-if="playlist.length === 0" description="播放列表为空" />
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useServiceStore } from '../stores'
import { tauriClient } from '../composables/useTauriCommands'
import {
  Headset,
  ArrowLeft,
  ArrowRight,
  VideoPlay,
  VideoPause,
  MuteNotification,
  List,
} from '@element-plus/icons-vue'

const serviceStore = useServiceStore()

// 状态
const playProgress = ref(0)
const volumeValue = ref(100)
const currentTime = ref(0)
const totalTime = ref(0)
const isMuted = ref(false)
const volumeBeforeMute = ref(100)

// 计算属性
const isConnected = computed(() => serviceStore.isConnected)
const isPlaying = computed(() => serviceStore.isPlaying)
const playlist = computed(() => serviceStore.playlist)
const currentIndex = computed(() => serviceStore.currentIndex)
const masterVolume = computed(() => serviceStore.masterVolume)

const currentTrackName = computed(() => {
  const track = playlist.value[currentIndex.value]
  return track?.title || null
})
const currentTrackArtist = computed(() => {
  const track = playlist.value[currentIndex.value]
  return track?.artist || null
})
const currentTrackAlbum = computed(() => {
  const track = playlist.value[currentIndex.value]
  return track?.album || null
})
const hasTrack = computed(() => {
  return currentIndex.value >= 0 &&
         currentIndex.value < playlist.value.length &&
         playlist.value.length > 0
})

// 监听主音量变化
watch(masterVolume, (volume) => {
  volumeValue.value = Math.round(volume * 100)
})

// 播放/暂停切换
async function togglePlay() {
  if (isPlaying.value) {
    await serviceStore.pause()
  } else {
    await serviceStore.play()
  }
  // 刷新状态
  await serviceStore.refreshAudioState()
}

// 上一首
async function previousTrack() {
  await serviceStore.previous()
  // 刷新状态
  await serviceStore.refreshAll()
}

// 下一首
async function nextTrack() {
  await serviceStore.next()
  // 刷新状态
  await serviceStore.refreshAll()
}

// 处理音量变化
async function handleVolumeChange(value: number) {
  await serviceStore.setMasterVolume(value / 100)
  // 如果音量大于0，取消静音状态
  if (value > 0 && isMuted.value) {
    isMuted.value = false
  }
  // 如果音量为0，设置为静音状态
  if (value === 0 && !isMuted.value) {
    isMuted.value = true
  }
}

// 切换静音
async function toggleMute() {
  if (isMuted.value) {
    // 取消静音，恢复之前的音量
    await serviceStore.setMasterVolume(volumeBeforeMute.value / 100)
    volumeValue.value = volumeBeforeMute.value
    isMuted.value = false
  } else {
    // 静音，保存当前音量并设置为0
    volumeBeforeMute.value = volumeValue.value
    await serviceStore.setMasterVolume(0)
    volumeValue.value = 0
    isMuted.value = true
  }
}

// 播放指定曲目（双击行）
async function handlePlayTrack(_row: any, _column: any, $index: number) {
  await serviceStore.playTrackAt($index)
}

async function handlePlayTrackByIndex(index: number) {
  await serviceStore.playTrackAt(index)
}

// 格式化时间
function formatTime(seconds: number): string {
  if (!seconds || seconds < 0) return '0:00'
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 格式化时长
function formatDuration(seconds?: number): string {
  if (seconds === undefined || seconds === null) return '-'
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 更新播放进度
async function updateProgress() {
  if (!isPlaying.value) {
    return
  }

  try {
    const progress = await tauriClient.getPlaybackProgress()
    currentTime.value = progress.position
    totalTime.value = progress.duration || 0
    playProgress.value = (progress.progress || 0) * 100
  } catch (e) {
    console.error('获取播放进度失败:', e)
    // 获取进度失败时，不清除当前值，只是记录错误
  }
}

// 定时更新播放进度
let progressInterval: ReturnType<typeof setInterval> | null = null
let initTimeoutId: ReturnType<typeof setTimeout> | null = null

function startProgressUpdate() {
  if (progressInterval) {
    clearInterval(progressInterval)
    }
  // 每 500ms 更新一次进度
  progressInterval = setInterval(() => {
    updateProgress()
  }, 500)
}

function stopProgressUpdate() {
  if (progressInterval) {
    clearInterval(progressInterval)
    progressInterval = null
  }
}

// 监听播放状态
watch(isPlaying, (playing) => {
  if (playing) {
    updateProgress() // 立即更新一次
    startProgressUpdate()
  } else {
    stopProgressUpdate()
  }
})

// 监听当前曲目变化，更新显示信息
watch(currentIndex, (newIndex, oldIndex) => {
  if (newIndex !== oldIndex && newIndex >= 0) {
    // 切换曲目时更新进度
    if (isPlaying.value) {
      updateProgress()
    }
  }
})

// 初始化
onMounted(async () => {
  if (isConnected.value) {
    await serviceStore.refreshAll()
    // 如果正在播放，立即更新进度并启动定时更新
    if (isPlaying.value) {
      updateProgress()
      startProgressUpdate()
    }
  }
  volumeValue.value = Math.round(masterVolume.value * 100)

  // 添加一个延迟检查，确保状态已正确加载
  initTimeoutId = setTimeout(async () => {
    if (isConnected.value) {
      await serviceStore.refreshAudioState()
      // 再次检查播放状态
      if (isPlaying.value && !progressInterval) {
        updateProgress()
        startProgressUpdate()
      }
    }
  }, 100)
})

onUnmounted(() => {
  stopProgressUpdate()
  // 清理初始化的 timeout
  if (initTimeoutId !== null) {
    clearTimeout(initTimeoutId)
    initTimeoutId = null
  }
})
</script>

<style scoped>
.playing-view {
  padding: 20px;
  max-width: 900px;
  margin: 0 auto;
}

.now-playing-card {
  text-align: center;
  margin-bottom: 20px;
}

.album-cover {
  position: relative;
  width: 200px;
  height: 200px;
  margin: 0 auto 30px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border-radius: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 10px 40px rgba(102, 126, 234, 0.3);
}

.cover-icon {
  color: rgba(255, 255, 255, 0.8);
}

.playing-indicator {
  position: absolute;
  bottom: 10px;
  right: 10px;
  color: #fff;
  font-size: 24px;
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.7;
    transform: scale(1.1);
  }
}

.track-info {
  margin-bottom: 30px;
}

.track-title {
  font-size: 28px;
  font-weight: 600;
  color: #303133;
  margin: 0 0 10px;
}

.track-artist,
.track-album {
  font-size: 16px;
  color: #909399;
  margin: 5px 0;
}

.progress-section {
  margin-bottom: 30px;
  padding: 0 40px;
}

.time-info {
  display: flex;
  justify-content: space-between;
  margin-top: 10px;
  font-size: 12px;
  color: #909399;
}

.playback-controls {
  margin-bottom: 30px;
}

.volume-control {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 15px;
}

.volume-icon-btn {
  padding: 8px;
  color: #909399;
  transition: all 0.3s;
}

.volume-icon-btn:hover:not(:disabled) {
  color: #409eff;
}

.volume-icon-btn.muted {
  color: #f56c6c;
}

.volume-icon-btn.muted:hover:not(:disabled) {
  color: #f78989;
}

.volume-icon {
  color: inherit;
}

.volume-text {
  min-width: 45px;
  text-align: right;
  color: #909399;
  font-size: 14px;
}

.playlist-card {
  margin-top: 20px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.playlist-track-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: #303133;
}

.playing-icon {
  color: #409eff;
  animation: pulse 1s infinite;
}
</style>
