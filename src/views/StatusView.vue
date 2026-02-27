<template>
  <div class="settings-view">
    <!-- 音乐播放器状态 -->
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <el-icon><MusicNote /></el-icon>
          <span>音频状态</span>
        </div>
      </template>
      <el-descriptions :column="2" border v-if="audioState">
        <el-descriptions-item label="音乐音量">
          {{ Math.round(audioState.music_volume * 100) }}%
        </el-descriptions-item>
        <el-descriptions-item label="广播音量">
          {{ Math.round(audioState.broadcast_volume * 100) }}%
        </el-descriptions-item>
        <el-descriptions-item label="主音量">
          {{ Math.round(audioState.master_volume * 100) }}%
        </el-descriptions-item>
        <el-descriptions-item label="闪避状态">
          <el-tag :type="audioState.is_ducking ? 'warning' : 'success'" size="small">
            {{ audioState.is_ducking ? '闪避中' : '正常' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="闪避音量">
          {{ Math.round(audioState.ducking_volume * 100) }}%
        </el-descriptions-item>
        <el-descriptions-item label="淡入/淡出时间">
          {{ audioState.ducking_fade_in_ms }}ms / {{ audioState.ducking_fade_out_ms }}ms
        </el-descriptions-item>
        <el-descriptions-item label="播放模式">
          {{ playModeLabel }}
        </el-descriptions-item>
        <el-descriptions-item label="播放列表长度">
          {{ audioState.playlist_length }} 首曲目
        </el-descriptions-item>
      </el-descriptions>
    </el-card>

    <!-- 服务信息 -->
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <el-icon><InfoFilled /></el-icon>
          <span>服务信息</span>
        </div>
      </template>
      <el-descriptions :column="2" border>
        <el-descriptions-item label="服务状态">
          <el-tag type="success" size="small">
            运行中
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="WebSocket 地址">
          {{ wsAddress }}
        </el-descriptions-item>
        <el-descriptions-item label="HTTP API 地址">
          {{ httpAddress }}
        </el-descriptions-item>
        <el-descriptions-item label="当前曲目">
          {{ currentTrack || '无' }}
        </el-descriptions-item>
      </el-descriptions>
    </el-card>

    <!-- 关于 -->
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <el-icon><Document /></el-icon>
          <span>关于</span>
        </div>
      </template>
      <div class="about-content">
        <h3>广播服务管理器</h3>
        <p>版本: 0.1.0</p>
        <p>基于 Tauri v2 + Vue 3 构建的桌面管理应用</p>
        <p>用于管理广播服务的播放列表、混音器和系统配置。</p>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useServiceStore } from '../stores'
import { tauriClient } from '../composables'
import {
  InfoFilled,
  Document,
} from '@element-plus/icons-vue'

// Store
const serviceStore = useServiceStore()

// 音频状态
const audioState = computed(() => serviceStore.audioState)

// 当前曲目 - 格式化为"序号 - 标题"
const currentTrack = computed(() => {
  const currentIndex = serviceStore.currentIndex
  const playlist = serviceStore.playlist
  if (currentIndex >= 0 && currentIndex < playlist.length) {
    const track = playlist[currentIndex]
    return `${currentIndex + 1} - ${track.title}`
  }
  return '无'
})

// 服务器配置
const wsPort = ref(8080)
const httpPort = ref(8081)
const bindAddress = ref('0.0.0.0')

// 判断是否为环回地址
const isLoopbackAddress = (addr: string): boolean => {
  const loopbackAddresses = ['localhost', '127.0.0.1', '::1']
  return loopbackAddresses.includes(addr)
}

// 判断是否为 IPv6 地址
const isIPv6 = (addr: string): boolean => {
  return addr.includes(':')
}

// 格式化 URL 中的地址（IPv6 需要方括号）
const formatUrlAddress = (addr: string): string => {
  // 环回地址显示为 localhost
  if (isLoopbackAddress(addr)) {
    return 'localhost'
  }
  // IPv6 地址需要方括号
  if (isIPv6(addr)) {
    return `[${addr}]`
  }
  return addr
}

// WebSocket 地址
const wsAddress = computed(() => {
  const addr = formatUrlAddress(bindAddress.value)
  return `ws://${addr}:${wsPort.value}/ws`
})

// HTTP API 地址
const httpAddress = computed(() => {
  const addr = formatUrlAddress(bindAddress.value)
  return `http://${addr}:${httpPort.value}/api`
})

// 播放模式标签
const playModeLabel = computed(() => {
  const mode = audioState.value?.play_mode
  switch (mode) {
    case 'sequential': return '顺序播放'
    case 'loop': return '列表循环'
    case 'single_loop': return '单曲循环'
    case 'shuffle': return '随机播放'
    default: return mode || '未知'
  }
})

// 定时刷新
let refreshInterval: ReturnType<typeof setInterval> | null = null

// 刷新状态
async function refreshState() {
  await serviceStore.refreshAudioState()
  // 同时获取服务器配置
  try {
    const config = await tauriClient.getServerConfig()
    bindAddress.value = config.bind_address
    wsPort.value = config.ws_port
    httpPort.value = config.http_port
  } catch (e) {
    console.error('获取服务器配置失败:', e)
  }
}

// 初始化
onMounted(() => {
  refreshState()
  refreshInterval = setInterval(refreshState, 5000)
})

onUnmounted(() => {
  if (refreshInterval !== null) {
    clearInterval(refreshInterval)
    refreshInterval = null
  }
})
</script>

<style scoped>
.settings-view {
  padding: 20px;
}

.settings-card {
  margin-bottom: 20px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.about-content h3 {
  margin: 0 0 10px;
  color: #303133;
}

.about-content p {
  margin: 5px 0;
  color: #606266;
}
</style>
