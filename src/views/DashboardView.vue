<template>
  <div class="dashboard-view">
    <!-- 服务状态卡片 -->
    <el-row :gutter="20" class="status-row">
      <el-col :span="6">
        <el-card class="status-card">
          <template #header>
            <div class="card-header">
              <el-icon><Monitor /></el-icon>
              <span>服务状态</span>
            </div>
          </template>
          <div class="status-content">
            <el-tag type="success" size="large">
              运行中
            </el-tag>
            <p class="port-info">WS: 8080 | HTTP: 8081</p>
          </div>
          <div class="status-actions">
            <el-button type="info" disabled>
              服务已自动启动
            </el-button>
          </div>
        </el-card>
      </el-col>

      <el-col :span="6">
        <el-card class="status-card">
          <template #header>
            <div class="card-header">
              <el-icon><Headset /></el-icon>
              <span>广播状态</span>
            </div>
          </template>
          <div class="status-content">
            <el-tag :type="serviceStore.isBroadcasting ? 'warning' : 'info'" size="large">
              {{ serviceStore.isBroadcasting ? '闪避中' : '正常' }}
            </el-tag>
          </div>
        </el-card>
      </el-col>

      <el-col :span="6">
        <el-card class="status-card">
          <template #header>
            <div class="card-header">
              <el-icon><MusicNote /></el-icon>
              <span>音乐状态</span>
            </div>
          </template>
          <div class="status-content">
            <el-tag :type="getMusicStateType()" size="large">
              {{ serviceStore.isPlaying ? 'playing' : 'stopped' }}
            </el-tag>
            <p class="volume-info">音量: {{ Math.round(serviceStore.musicVolume * 100) }}%</p>
          </div>
        </el-card>
      </el-col>

      <el-col :span="6">
        <el-card class="status-card">
          <template #header>
            <div class="card-header">
              <el-icon><User /></el-icon>
              <span>播放列表</span>
            </div>
          </template>
          <div class="status-content">
            <div class="client-count">{{ serviceStore.playlist.length }}</div>
            <p class="client-info">首曲目</p>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 快速控制面板 -->
    <el-card class="control-panel">
      <template #header>
        <div class="card-header">
          <el-icon><Operation /></el-icon>
          <span>快速控制</span>
        </div>
      </template>
      <el-row :gutter="20">
        <el-col :span="12">
          <div class="control-section">
            <h4>音乐播放</h4>
            <el-button-group>
              <el-button @click="serviceStore.previous()" :disabled="!serviceIsRunning">
                <el-icon><ArrowLeft /></el-icon>
              </el-button>
              <el-button
                @click="serviceStore.isPlaying ? serviceStore.pause() : serviceStore.play()"
                :disabled="!serviceIsRunning"
              >
                <el-icon>
                  <VideoPause v-if="serviceStore.isPlaying" />
                  <VideoPlay v-else />
                </el-icon>
              </el-button>
              <el-button @click="serviceStore.next()" :disabled="!serviceIsRunning">
                <el-icon><ArrowRight /></el-icon>
              </el-button>
              <el-button @click="serviceStore.stop()" :disabled="!serviceIsRunning">
                <el-icon><CloseBold /></el-icon>
              </el-button>
            </el-button-group>
          </div>
        </el-col>
        <el-col :span="12">
          <div class="control-section">
            <h4>主音量</h4>
            <el-slider
              v-model="masterVolume"
              :min="0"
              :max="100"
              :disabled="!serviceIsRunning"
              @change="handleVolumeChange"
            />
          </div>
        </el-col>
      </el-row>
    </el-card>

    <!-- 当前播放 -->
    <el-card class="current-track" v-if="currentTrackName">
      <template #header>
        <div class="card-header">
          <el-icon><Headset /></el-icon>
          <span>当前播放</span>
        </div>
      </template>
      <div class="track-info">
        <h3>{{ currentTrackName }}</h3>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useServiceStore } from '../stores'
import { useServiceManager } from '../composables'
import {
  Monitor,
  Headset,
  Headset as MusicNote,
  User,
  Operation,
  ArrowLeft,
  ArrowRight,
  VideoPause,
  VideoPlay,
  CloseBold,
} from '@element-plus/icons-vue'

// Store
const serviceStore = useServiceStore()

// 服务管理（服务总是运行中）
const { isRunning: serviceIsRunning } = useServiceManager()

// 主音量
const masterVolume = ref(100)

// 当前曲目名称 - 从播放列表获取，格式化为"序号 - 标题"
const currentTrackName = computed(() => {
  const currentIndex = serviceStore.currentIndex
  const playlist = serviceStore.playlist
  if (currentIndex >= 0 && currentIndex < playlist.length) {
    const track = playlist[currentIndex]
    return `${currentIndex + 1} - ${track.title}`
  }
  return '无'
})

// 获取音乐状态类型
function getMusicStateType(): '' | 'success' | 'warning' | 'info' | 'danger' {
  if (serviceStore.isPlaying) {
    return 'success'
  }
  return 'info'
}

// 处理音量变化
async function handleVolumeChange(value: number) {
  await serviceStore.setMasterVolume(value / 100)
}

// 监听主音量变化
watch(
  () => serviceStore.masterVolume,
  (volume) => {
    masterVolume.value = Math.round(volume * 100)
  },
  { immediate: true }
)

// 初始化
onMounted(async () => {
  if (serviceIsRunning.value) {
    await serviceStore.refreshAll()
  }
})
</script>

<style scoped>
.dashboard-view {
  padding: 20px;
}

.status-row :deep(.el-col) > div {
  height: 100%;
  display: flex;
}

.status-card {
  text-align: center;
  width: 100%;
  display: flex;
  flex-direction: column;
}

.status-card :deep(.el-card__body) {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-content {
  padding: 20px 0;
}

.status-content .el-tag {
  margin-bottom: 10px;
}

.port-info,
.volume-info,
.client-info {
  color: #909399;
  font-size: 14px;
  margin-top: 10px;
}

.client-count {
  font-size: 48px;
  font-weight: bold;
  color: #409eff;
}

.status-actions {
  margin-top: 15px;
}

.control-panel {
  margin-top: 20px;
}

.control-section {
  text-align: center;
}

.control-section h4 {
  margin-bottom: 15px;
  color: #606266;
}

.current-track {
  margin-top: 20px;
}

.track-info h3 {
  margin: 0;
  color: #303133;
}

.track-info p {
  margin: 8px 0 0;
  color: #909399;
}
</style>
