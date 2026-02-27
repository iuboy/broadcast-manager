<template>
  <div class="mixer-view">
    <!-- 混音器控制 -->
    <el-row :gutter="20">
      <!-- 主音量 -->
      <el-col :span="8">
        <el-card class="mixer-card">
          <template #header>
            <div class="card-header">
              <el-icon><Speaker /></el-icon>
              <span>主音量</span>
            </div>
          </template>
          <div class="mixer-control">
            <div class="volume-display">{{ Math.round(masterVolumeLocal) }}%</div>
            <el-slider
              v-model="masterVolumeLocal"
              vertical
              height="200px"
              :min="0"
              :max="100"
              @change="handleMasterVolumeChange"
              :disabled="!isConnected"
            />
          </div>
        </el-card>
      </el-col>

      <!-- 音乐音量 -->
      <el-col :span="8">
        <el-card class="mixer-card">
          <template #header>
            <div class="card-header">
              <el-icon><MusicNote /></el-icon>
              <span>音乐音量</span>
            </div>
          </template>
          <div class="mixer-control">
            <div class="volume-display">{{ Math.round(musicVolumeLocal) }}%</div>
            <el-slider
              v-model="musicVolumeLocal"
              vertical
              height="200px"
              :min="0"
              :max="100"
              @change="handleMusicVolumeChange"
              :disabled="!isConnected"
            />
          </div>
        </el-card>
      </el-col>

      <!-- 广播音量 -->
      <el-col :span="8">
        <el-card class="mixer-card">
          <template #header>
            <div class="card-header">
              <el-icon><Microphone /></el-icon>
              <span>广播音量</span>
            </div>
          </template>
          <div class="mixer-control">
            <div class="volume-display">{{ Math.round(broadcastVolumeLocal) }}%</div>
            <el-slider
              v-model="broadcastVolumeLocal"
              vertical
              height="200px"
              :min="0"
              :max="100"
              @change="handleBroadcastVolumeChange"
              :disabled="!isConnected"
            />
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 音频闪避控制 -->
    <el-card class="ducking-control">
      <template #header>
        <div class="card-header">
          <el-icon><Operation /></el-icon>
          <span>音频闪避控制</span>
        </div>
      </template>
      <el-row :gutter="20">
        <el-col :span="16">
          <el-row :gutter="20">
            <el-col :span="8">
              <div class="ducking-item">
                <span class="ducking-label">启用闪避</span>
                <el-switch
                  v-model="duckingEnabledLocal"
                  @change="handleDuckingEnabledChange"
                  :disabled="!isConnected"
                />
              </div>
            </el-col>
            <el-col :span="8">
              <div class="ducking-item">
                <span class="ducking-label">闪避音量: {{ Math.round(duckingVolumeLocal ) }}%</span>
                <el-slider
                  v-model="duckingVolumeLocal"
                  :min="0"
                  :max="100"
                  @change="handleDuckingVolumeChange"
                  :disabled="!isConnected || !duckingEnabledLocal"
                />
              </div>
            </el-col>
            <el-col :span="8">
              <div class="ducking-item">
                <span class="ducking-label">状态</span>
                <el-tag :type="isDucking ? 'warning' : 'success'" size="small">
                  {{ isDucking ? '闪避中' : '正常' }}
                </el-tag>
              </div>
            </el-col>
          </el-row>
        </el-col>
        <el-col :span="8">
          <div class="ducking-info">
            <p class="ducking-description">
              音频闪避功能可以在广播时自动降低背景音乐音量，使广播更加清晰。
            </p>
          </div>
        </el-col>
      </el-row>
    </el-card>

    <!-- 淡入/淡出时间配置 -->
    <el-card class="fade-config">
      <template #header>
        <div class="card-header">
          <el-icon><Timer /></el-icon>
          <span>淡入/淡出时间配置</span>
        </div>
      </template>
      <el-row :gutter="20">
        <el-col :span="12">
          <div class="fade-item">
            <div class="fade-header">
              <span class="fade-label">淡入时间 (音乐恢复)</span>
              <span class="fade-value">{{ fadeInMsLocal }} ms</span>
            </div>
            <el-slider
              v-model="fadeInMsLocal"
              :min="0"
              :max="2000"
              :step="50"
              @change="handleFadeInMsChange"
              :disabled="!isConnected || !duckingEnabledLocal"
            />
          </div>
        </el-col>
        <el-col :span="12">
          <div class="fade-item">
            <div class="fade-header">
              <span class="fade-label">淡出时间 (开始闪避)</span>
              <span class="fade-value">{{ fadeOutMsLocal }} ms</span>
            </div>
            <el-slider
              v-model="fadeOutMsLocal"
              :min="0"
              :max="2000"
              :step="50"
              @change="handleFadeOutMsChange"
              :disabled="!isConnected || !duckingEnabledLocal"
            />
          </div>
        </el-col>
      </el-row>
    </el-card>

    <!-- 状态监控 -->
    <el-card class="status-monitor">
      <template #header>
        <div class="card-header">
          <el-icon><DataLine /></el-icon>
          <span>状态监控</span>
        </div>
      </template>
      <el-descriptions :column="3" border>
        <el-descriptions-item label="主音量">
          {{ Math.round(masterVolume * 100) }}%
        </el-descriptions-item>
        <el-descriptions-item label="音乐音量">
          {{ Math.round(musicVolume * 100) }}%
        </el-descriptions-item>
        <el-descriptions-item label="广播音量">
          {{ Math.round(broadcastVolume * 100) }}%
        </el-descriptions-item>
        <el-descriptions-item label="闪避状态">
          <el-tag :type="isDucking ? 'warning' : 'success'" size="small">
            {{ isDucking ? '闪避中' : '正常' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="闪避启用">
          <el-tag :type="duckingEnabled ? 'success' : 'info'" size="small">
            {{ duckingEnabled ? '已启用' : '已禁用' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="连接状态">
          <el-tag :type="isConnected ? 'success' : 'danger'" size="small">
            {{ isConnected ? '已连接' : '未连接' }}
          </el-tag>
        </el-descriptions-item>
      </el-descriptions>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useServiceStore } from '../stores'
import { ElMessage } from 'element-plus'
import {
  Microphone,
  Microphone as Speaker,
  Headset as MusicNote,
  Operation,
  DataLine,
  Timer,
} from '@element-plus/icons-vue'

// Store
const serviceStore = useServiceStore()

// 计算属性 - 从 Store 获取状态
const isConnected = computed(() => serviceStore.isConnected)
const masterVolume = computed(() => serviceStore.masterVolume)
const musicVolume = computed(() => serviceStore.musicVolume)
const broadcastVolume = computed(() => serviceStore.broadcastVolume)
const duckingEnabled = computed(() => serviceStore.duckingEnabled)
const isDucking = computed(() => serviceStore.isBroadcasting)

// 本地状态用于滑块 - 使用默认初始值，会在数据加载后更新
const masterVolumeLocal = ref(100)
const musicVolumeLocal = ref(100)
const broadcastVolumeLocal = ref(100)
const duckingEnabledLocal = ref(false)
const duckingVolumeLocal = ref(30)
const fadeInMsLocal = ref(500)
const fadeOutMsLocal = ref(1000)

// 定时刷新
let refreshInterval: number | null = null

// 刷新状态
async function refreshState() {
  await serviceStore.refreshAudioState()
}

// 监听 audioState 变化
watch(
  () => serviceStore.audioState,
  (state) => {
    // 当 state 为 null 时使用默认值，否则使用实际值
    masterVolumeLocal.value = Math.round((state?.master_volume ?? 1.0) * 100)
    musicVolumeLocal.value = Math.round((state?.music_volume ?? 1.0) * 100)
    broadcastVolumeLocal.value = Math.round((state?.broadcast_volume ?? 1.0) * 100)
    duckingEnabledLocal.value = state?.ducking_enabled ?? false
    duckingVolumeLocal.value = Math.round((state?.ducking_volume ?? 0.3) * 100)
    fadeInMsLocal.value = state?.ducking_fade_in_ms ?? 500
    fadeOutMsLocal.value = state?.ducking_fade_out_ms ?? 1000
  },
  { immediate: true }
)

// 处理音量变化
async function handleMasterVolumeChange(value: number) {
  await serviceStore.setMasterVolume(value / 100)
}

async function handleMusicVolumeChange(value: number) {
  await serviceStore.setMusicVolume(value / 100)
}

async function handleBroadcastVolumeChange(value: number) {
  await serviceStore.setBroadcastVolume(value / 100)
}

// 音频闪避控制
async function handleDuckingEnabledChange(enabled: boolean) {
  try {
    await serviceStore.setDuckingEnabled(enabled)
    ElMessage.success(enabled ? '闪避功能已启用' : '闪避功能已禁用')
  } catch (e) {
    ElMessage.error('设置闪避功能失败')
    // 恢复原值
    duckingEnabledLocal.value = !enabled
  }
}

async function handleDuckingVolumeChange(value: number) {
  try {
    await serviceStore.setDuckingVolume(value / 100)
  } catch (e) {
    ElMessage.error('设置闪避音量失败')
  }
}

async function handleFadeInMsChange(value: number) {
  try {
    await serviceStore.setDuckingFadeInMs(value)
  } catch (e) {
    ElMessage.error('设置淡入时间失败')
  }
}

async function handleFadeOutMsChange(value: number) {
  try {
    await serviceStore.setDuckingFadeOutMs(value)
  } catch (e) {
    ElMessage.error('设置淡出时间失败')
  }
}

// 生命周期
onMounted(async () => {
  await refreshState()
  refreshInterval = window.setInterval(refreshState, 2000)
})

onUnmounted(() => {
  if (refreshInterval) {
    window.clearInterval(refreshInterval)
  }
})
</script>

<style scoped>
.mixer-view {
  padding: 20px;
}

.mixer-card {
  text-align: center;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.mixer-control {
  padding: 20px;
  display: flex;
  flex-direction: column;
  align-items: center;
}

.volume-display {
  font-size: 24px;
  font-weight: bold;
  color: #409eff;
  margin-bottom: 20px;
}

.ducking-control {
  margin-top: 20px;
}

.ducking-item {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px;
  background: #f5f7fa;
  border-radius: 8px;
}

.ducking-label {
  font-weight: 500;
  color: #606266;
}

.ducking-info {
  display: flex;
  align-items: center;
}

.ducking-description {
  color: #606266;
  margin: 0;
  line-height: 1.6;
}

.fade-config {
  margin-top: 20px;
}

.fade-item {
  padding: 15px;
  background: #f5f7fa;
  border-radius: 8px;
}

.fade-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 15px;
}

.fade-label {
  font-weight: 500;
  color: #606266;
}

.fade-value {
  color: #409eff;
  font-weight: bold;
}

.status-monitor {
  margin-top: 20px;
}
</style>
