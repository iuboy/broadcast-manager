/**
 * 服务管理
 *
 * 内嵌服务总是自动运行，不需要手动启动/停止
 */

import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 服务状态类型
type ServiceState = 'running' | 'error'

// 服务状态信息
interface ServiceStatus {
  status: ServiceState
  state: string
  port: number
  error: string | null
}

export function useServiceManager() {
  // 状态 - 服务总是运行中
  const status = ref<ServiceStatus>({
    status: 'running',
    state: 'running',
    port: 8081,
    error: null,
  })

  const isLoading = ref(false)

  // 计算属性
  const isRunning = computed(() => status.value.status === 'running')
  const isStopped = computed(() => false) // 服务不会停止
  const hasError = computed(() => status.value.status === 'error')

  // 获取状态
  async function getStatus(): Promise<ServiceStatus> {
    try {
      const result = await invoke<ServiceStatus>('get_service_status')
      status.value = result
      return result
    } catch (error) {
      console.error('获取服务状态失败:', error)
      status.value = {
        status: 'running', // 默认为运行
        state: 'running',
        port: 8081,
        error: null,
      }
      return status.value
    }
  }

  // 启动服务（服务已自动运行，此函数仅用于兼容）
  async function start(): Promise<ServiceStatus> {
    isLoading.value = true
    try {
      const result = await getStatus()
      status.value = result
      return result
    } catch (error) {
      console.error('获取服务状态失败:', error)
      status.value = {
        status: 'error',
        state: 'error',
        port: 8081,
        error: error instanceof Error ? error.message : String(error),
      }
      return status.value
    } finally {
      isLoading.value = false
    }
  }

  // 停止服务（服务不能停止，此函数仅用于兼容）
  async function stop(): Promise<ServiceStatus> {
    // 服务是内嵌的，不能停止
    console.warn('内嵌服务无法停止，请关闭应用退出')
    return status.value
  }

  // 切换服务状态（仅返回运行状态）
  async function toggle(): Promise<ServiceStatus> {
    return start()
  }

  // 生命周期
  onMounted(async () => {
    await getStatus()
  })

  return {
    // 状态
    status,
    isLoading,

    // 计算属性
    isRunning,
    isStopped,
    hasError,

    // 方法
    getStatus,
    start,
    stop,
    toggle,
  }
}

export type { ServiceStatus, ServiceState }
