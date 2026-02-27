<template>
  <div class="playlist-view">
    <!-- 工具栏 -->
    <el-card class="toolbar">
      <el-row :gutter="20" align="middle">
        <el-col :span="12">
          <el-button-group>
            <el-button @click="showScanDialog = true" :disabled="!isConnected">
              <el-icon><FolderAdd /></el-icon>
              扫描目录
            </el-button>
            <el-button @click="clearPlaylist" :disabled="!isConnected || playlist.length === 0">
              <el-icon><Delete /></el-icon>
              清空列表
            </el-button>
          </el-button-group>
        </el-col>
        <el-col :span="12" style="text-align: right">
          <el-select v-model="playModeValue" @change="handlePlayModeChange" :disabled="!isConnected" style="width: 150px">
            <el-option label="顺序播放" value="sequential" />
            <el-option label="列表循环" value="loop" />
            <el-option label="单曲循环" value="single_loop" />
            <el-option label="随机播放" value="shuffle" />
          </el-select>
        </el-col>
      </el-row>
    </el-card>

    <!-- 播放列表 -->
    <el-card class="playlist-container">
      <el-table
        :data="playlist"
        highlight-current-row
        :current-row-key="currentIndex"
        @row-dblclick="handleRowDblClick"
        style="width: 100%"
      >
        <el-table-column type="index" width="50" label="#" />
        <el-table-column prop="title" label="标题" min-width="200">
          <template #default="{ row, $index }">
            <div class="track-title">
              <el-icon v-if="$index === currentIndex" class="playing-icon"><Headset /></el-icon>
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
        <el-table-column label="操作" width="120" fixed="right">
          <template #default="{ row: _row, $index }">
            <el-button-group size="small">
              <el-button @click.stop="playTrack($index)" :disabled="!isConnected">
                <el-icon><VideoPlay /></el-icon>
              </el-button>
              <el-button @click.stop="removeTrack($index)" :disabled="!isConnected" type="danger">
                <el-icon><Delete /></el-icon>
              </el-button>
            </el-button-group>
          </template>
        </el-table-column>
      </el-table>

      <el-empty v-if="playlist.length === 0" description="播放列表为空" />
    </el-card>

    <!-- 扫描目录对话框 -->
    <el-dialog v-model="showScanDialog" title="扫描音乐目录" width="500px">
      <el-form>
        <el-form-item label="目录路径">
          <el-input v-model="scanDirPath" placeholder="请输入音乐目录路径或点击右侧按钮选择">
            <template #append>
              <el-button @click="selectDirectory" :icon="Folder">选择</el-button>
            </template>
          </el-input>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showScanDialog = false">取消</el-button>
        <el-button type="primary" @click="handleScan" :loading="isScanning">扫描</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useServiceStore } from '../stores'
import { ElMessage } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import {
  FolderAdd,
  Delete,
  Headset,
  VideoPlay,
  Folder,
} from '@element-plus/icons-vue'

const router = useRouter()

// Store
const serviceStore = useServiceStore()

// 状态
const showScanDialog = ref(false)
const scanDirPath = ref('')
const isScanning = ref(false)

// 计算属性 - 从 Store 获取状态
const playlist = computed(() => serviceStore.playlist)
const currentIndex = computed(() => serviceStore.currentIndex)
const isConnected = computed(() => serviceStore.isConnected)
const playModeValue = computed({
  get: () => serviceStore.playMode,
  set: () => {} // handled by change event
})

// 格式化时长
function formatDuration(seconds?: number): string {
  if (seconds === undefined || seconds === null) return '-'
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 播放指定曲目
async function playTrack(index: number) {
  await serviceStore.playTrackAt(index)
  // 跳转到正在播放页面
  router.push('/playing')
}

// 双击播放
function handleRowDblClick(_row: { id: string }, _column: any, $index: number) {
  playTrack($index)
}

// 移除曲目
async function removeTrack(index: number) {
  await serviceStore.removeFromPlaylist(index)
  ElMessage.success('已移除')
}

// 清空播放列表
async function clearPlaylist() {
  await serviceStore.clearPlaylist()
  ElMessage.success('已清空')
}

// 切换播放模式
async function handlePlayModeChange(mode: string) {
  try {
    await serviceStore.setPlayMode(mode as 'sequential' | 'loop' | 'single_loop' | 'shuffle')
    ElMessage.success('播放模式已更改')
  } catch (e) {
    ElMessage.error('更改播放模式失败')
  }
}

// 扫描目录
async function handleScan() {
  if (!scanDirPath.value) {
    ElMessage.warning('请输入目录路径')
    return
  }

  isScanning.value = true
  try {
    const count = await serviceStore.scanDirectory(scanDirPath.value)
    ElMessage.success(`已添加 ${count} 个音乐文件`)
    showScanDialog.value = false
    scanDirPath.value = ''
  } catch (e) {
    ElMessage.error('扫描失败: ' + (e as Error).message)
  } finally {
    isScanning.value = false
  }
}

// 选择目录
async function selectDirectory() {
  try {
    console.log('正在打开目录选择对话框...')
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择音乐目录',
    }) as string | string[] | null

    console.log('选择的目录 (原始):', selected, '类型:', typeof selected)

    if (selected) {
      // Tauri v2 dialog 返回的可能是 string 或 null
      if (typeof selected === 'string') {
        scanDirPath.value = selected
        ElMessage.success('已选择目录: ' + selected)
      } else if (Array.isArray(selected) && selected.length > 0) {
        scanDirPath.value = selected[0]
        ElMessage.success('已选择目录: ' + selected[0])
      }
    } else {
      console.log('用户取消了选择')
    }
  } catch (e) {
    console.error('选择目录失败:', e)
    ElMessage.error('选择目录失败: ' + (e as Error).message)
  }
}

// 初始化
onMounted(async () => {
  if (isConnected.value) {
    await serviceStore.refreshPlaylist()
  }
})

// 监听连接状态
watch(isConnected, async (connected) => {
  if (connected) {
    await serviceStore.refreshPlaylist()
  }
})
</script>

<style scoped>
.playlist-view {
  padding: 20px;
}

.toolbar {
  margin-bottom: 20px;
}

.playlist-container {
  min-height: 400px;
}

.track-title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.playing-icon {
  color: #409eff;
  animation: pulse 1s infinite;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}
</style>
