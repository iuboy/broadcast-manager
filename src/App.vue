<template>
  <el-container class="app-container">
    <!-- 侧边栏 -->
    <el-aside width="200px" class="app-aside">
      <div class="logo">
        <h1>广播管理</h1>
      </div>
      <el-menu
        :default-active="currentRoute"
        router
        :collapse="false"
        class="app-menu"
      >
        <el-menu-item index="/">
          <el-icon><DataBoard /></el-icon>
          <span>仪表盘</span>
        </el-menu-item>
        <el-menu-item index="/playlist">
          <el-icon><List /></el-icon>
          <span>播放列表</span>
        </el-menu-item>
        <el-menu-item index="/playing">
          <el-icon><Headset /></el-icon>
          <span>正在播放</span>
        </el-menu-item>
        <el-menu-item index="/mixer">
          <el-icon><Operation /></el-icon>
          <span>混音器</span>
        </el-menu-item>
        <el-menu-item index="/status">
          <el-icon><Monitor /></el-icon>
          <span>状态</span>
        </el-menu-item>
        <el-menu-item index="/settings">
          <el-icon><Setting /></el-icon>
          <span>设置</span>
        </el-menu-item>
      </el-menu>
    </el-aside>

    <!-- 主内容区 -->
    <el-container>
      <!-- 顶部状态栏 -->
      <el-header class="app-header">
        <div class="header-left">
          <span class="page-title">{{ currentPageTitle }}</span>
        </div>
        <div class="header-right">
          <el-tag :type="serviceIsRunning ? 'success' : 'danger'" size="small">
            {{ serviceIsRunning ? '服务运行中' : '服务已停止' }}
          </el-tag>
          <el-tag v-if="serviceStore.isConnected" type="info" size="small">
            端口: 8080
          </el-tag>
        </div>
      </el-header>

      <!-- 内容区 -->
      <el-main class="app-main">
        <router-view />
      </el-main>

      <!-- 底部状态栏 -->
      <el-footer class="app-footer" height="32px">
        <span>广播服务管理器 v0.1.0</span>
        <span v-if="serviceStore.audioState">
          | {{ serviceStore.audioState.is_playing ? '播放中' : '已停止' }}
        </span>
      </el-footer>
    </el-container>
  </el-container>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useServiceManager } from './composables'
import { useServiceStore } from './stores'
import {
  DataBoard,
  List,
  Operation,
  Headset,
  Monitor,
  Setting,
} from '@element-plus/icons-vue'

const route = useRoute()
const serviceStore = useServiceStore()
const { isRunning: serviceIsRunning } = useServiceManager()

// 当前路由
const currentRoute = computed(() => route.path)

// 页面标题
const currentPageTitle = computed(() => {
  const meta = route.meta
  return (meta?.title as string) || '广播管理'
})

// 定期刷新状态
let refreshIntervalId: ReturnType<typeof setInterval> | null = null

onMounted(() => {
  // 每 5 秒刷新一次状态
  refreshIntervalId = setInterval(() => {
    if (serviceIsRunning.value) {
      serviceStore.refreshAudioState()
    }
  }, 5000)
})

onUnmounted(() => {
  if (refreshIntervalId !== null) {
    clearInterval(refreshIntervalId)
    refreshIntervalId = null
  }
})

// 监听服务状态
watch(serviceIsRunning, async (running) => {
  if (running) {
    await serviceStore.refreshAll()
  }
})
</script>

<style>
/* 全局样式 */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100%;
  width: 100%;
  overflow: hidden;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
}
</style>

<style scoped>
.app-container {
  height: 100vh;
  width: 100vw;
}

.app-aside {
  background-color: #304156;
  color: #fff;
}

.logo {
  height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.logo h1 {
  font-size: 18px;
  font-weight: 600;
  color: #fff;
}

.app-menu {
  border-right: none;
  background-color: #304156;
}

.app-menu .el-menu-item {
  color: #bfcbd9;
}

.app-menu .el-menu-item:hover,
.app-menu .el-menu-item.is-active {
  background-color: #263445;
  color: #409eff;
}

.app-header {
  background-color: #fff;
  border-bottom: 1px solid #dcdfe6;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
}

.header-left .page-title {
  font-size: 18px;
  font-weight: 600;
  color: #303133;
}

.header-right {
  display: flex;
  gap: 10px;
}

.app-main {
  background-color: #f0f2f5;
  overflow-y: auto;
}

.app-footer {
  background-color: #fff;
  border-top: 1px solid #dcdfe6;
  display: flex;
  align-items: center;
  padding: 0 20px;
  font-size: 12px;
  color: #909399;
  gap: 5px;
}
</style>
