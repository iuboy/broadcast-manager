/**
 * 服务状态 Store
 *
 * 管理广播服务的全局状态，使用 Tauri 命令通信
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { tauriClient, type AudioState, type PlaylistItem } from '../composables/useTauriCommands'

export const useServiceStore = defineStore('service', () => {
  // 状态
  const isConnected = ref(true) // Tauri 命令总是可用的
  const audioState = ref<AudioState | null>(null)
  const playlist = ref<PlaylistItem[]>([])

  // 加载状态
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  // 计算属性
  const isPlaying = computed(() => audioState.value?.is_playing ?? false)
  const isBroadcasting = computed(() => audioState.value?.is_ducking ?? false)
  const currentTrack = computed(() =>
    audioState.value?.current_track ?? null
  )
  const currentIndex = computed(() => {
    // 如果没有音频状态，返回 -1
    if (!audioState.value) return -1
    // 如果播放列表为空，返回 -1
    if (playlist.value.length === 0) return -1
    // 如果索引超出范围，返回 -1
    const idx = audioState.value.current_index ?? 0
    return idx < playlist.value.length ? idx : -1
  })

  // 音量相关
  const masterVolume = computed(() => audioState.value?.master_volume ?? 1.0)
  const musicVolume = computed(() => audioState.value?.music_volume ?? 1.0)
  const broadcastVolume = computed(() => audioState.value?.broadcast_volume ?? 1.0)

  // 闪避相关
  const duckingEnabled = computed(() => audioState.value?.ducking_enabled ?? false)
  const duckingVolume = computed(() => audioState.value?.ducking_volume ?? 0.3)
  const duckingFadeInMs = computed(() => audioState.value?.ducking_fade_in_ms ?? 500)
  const duckingFadeOutMs = computed(() => audioState.value?.ducking_fade_out_ms ?? 1000)

  // 播放模式
  const playMode = computed(() => audioState.value?.play_mode ?? 'loop')

  // 刷新音频状态
  async function refreshAudioState() {
    try {
      const state = await tauriClient.getAudioState()
      audioState.value = state
      isConnected.value = true
      error.value = null
    } catch (e) {
      isConnected.value = false
      error.value = e instanceof Error ? e.message : '获取状态失败'
    }
  }

  // 刷新播放列表
  async function refreshPlaylist() {
    try {
      const items = await tauriClient.getPlaylist()
      playlist.value = items
    } catch (e) {
      console.error('刷新播放列表失败:', e)
      error.value = e instanceof Error ? e.message : '刷新播放列表失败'
    }
  }

  // 刷新所有状态
  async function refreshAll() {
    isLoading.value = true
    try {
      await Promise.all([
        refreshAudioState(),
        refreshPlaylist(),
      ])
    } finally {
      isLoading.value = false
    }
  }

  // ===== 播放控制 =====

  async function play() {
    try {
      await tauriClient.play()
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '播放失败'
    }
  }

  async function pause() {
    try {
      await tauriClient.pause()
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '暂停失败'
    }
  }

  async function stop() {
    try {
      await tauriClient.stop()
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '停止失败'
    }
  }

  async function next() {
    try {
      await tauriClient.next()
      await Promise.all([refreshAudioState(), refreshPlaylist()])
    } catch (e) {
      error.value = e instanceof Error ? e.message : '切换失败'
    }
  }

  async function previous() {
    try {
      await tauriClient.previous()
      await Promise.all([refreshAudioState(), refreshPlaylist()])
    } catch (e) {
      error.value = e instanceof Error ? e.message : '切换失败'
    }
  }

  async function playTrackAt(index: number) {
    try {
      await tauriClient.playTrackAt(index)
      await Promise.all([refreshAudioState(), refreshPlaylist()])
    } catch (e) {
      error.value = e instanceof Error ? e.message : '播放失败'
    }
  }

  // ===== 音量控制 =====

  async function setMasterVolume(volume: number) {
    try {
      await tauriClient.setMasterVolume(volume)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置音量失败'
    }
  }

  async function setMusicVolume(volume: number) {
    try {
      await tauriClient.setMusicVolume(volume)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置音量失败'
    }
  }

  async function setBroadcastVolume(volume: number) {
    try {
      await tauriClient.setBroadcastVolume(volume)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置音量失败'
    }
  }

  // ===== 闪避控制 =====

  async function setDuckingEnabled(enabled: boolean) {
    try {
      await tauriClient.setDuckingEnabled(enabled)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置闪避启用失败'
    }
  }

  async function startDucking() {
    try {
      await tauriClient.startDucking()
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '启动闪避失败'
    }
  }

  async function stopDucking() {
    try {
      await tauriClient.stopDucking()
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '停止闪避失败'
    }
  }

  async function setDuckingVolume(volume: number) {
    try {
      await tauriClient.setDuckingVolume(volume)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置闪避音量失败'
    }
  }

  async function setDuckingFadeInMs(ms: number) {
    try {
      await tauriClient.setDuckingFadeInMs(ms)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置淡入时间失败'
    }
  }

  async function setDuckingFadeOutMs(ms: number) {
    try {
      await tauriClient.setDuckingFadeOutMs(ms)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置淡出时间失败'
    }
  }

  // ===== 播放列表管理 =====

  async function addToPlaylist(path: string) {
    try {
      await tauriClient.addTrack(path)
      await refreshPlaylist()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '添加失败'
    }
  }

  async function removeFromPlaylist(index: number) {
    try {
      await tauriClient.removeTrack(index)
      await refreshPlaylist()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '移除失败'
    }
  }

  async function clearPlaylist() {
    try {
      await tauriClient.clearPlaylist()
      await refreshPlaylist()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '清空失败'
    }
  }

  async function setPlayMode(mode: 'sequential' | 'loop' | 'single_loop' | 'shuffle') {
    try {
      await tauriClient.setPlayMode(mode)
      await refreshAudioState()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '设置播放模式失败'
    }
  }

  async function scanDirectory(path: string) {
    try {
      const count = await tauriClient.scanDirectory(path)
      await refreshPlaylist()
      return count
    } catch (e) {
      error.value = e instanceof Error ? e.message : '扫描目录失败'
      return 0
    }
  }

  return {
    // 状态
    isConnected,
    audioState,
    playlist,
    isLoading,
    error,

    // 计算属性
    isPlaying,
    isBroadcasting,
    currentTrack,
    currentIndex,
    masterVolume,
    musicVolume,
    broadcastVolume,
    duckingEnabled,
    duckingVolume,
    duckingFadeInMs,
    duckingFadeOutMs,
    playMode,

    // 方法
    refreshAudioState,
    refreshPlaylist,
    refreshAll,
    play,
    pause,
    stop,
    next,
    previous,
    playTrackAt,
    setMasterVolume,
    setMusicVolume,
    setBroadcastVolume,
    setDuckingEnabled,
    startDucking,
    stopDucking,
    setDuckingVolume,
    setDuckingFadeInMs,
    setDuckingFadeOutMs,
    addToPlaylist,
    removeFromPlaylist,
    clearPlaylist,
    setPlayMode,
    scanDirectory,
  }
})
