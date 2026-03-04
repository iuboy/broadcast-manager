/**
 * Tauri 命令客户端
 *
 * 直接使用 Tauri 命令与后端通信，替代 HTTP API
 */

import { invoke } from '@tauri-apps/api/core'

// ===== 类型定义 =====

/** 播放列表项 */
export interface PlaylistItem {
  id: string
  title: string
  artist?: string
  album?: string
  /** 时长（秒） */
  duration?: number
  path?: string
}

/** 音频状态 */
export interface AudioState {
  is_playing: boolean
  current_track: string | null
  music_volume: number
  broadcast_volume: number
  master_volume: number
  is_ducking: boolean
  ducking_enabled: boolean
  ducking_volume: number
  ducking_fade_in_ms: number
  ducking_fade_out_ms: number
  playlist_length: number
  current_index: number
  play_mode: string
}

/** 播放进度 */
export interface PlaybackProgress {
  /** 当前播放位置（秒） */
  position: number
  /** 音频总时长（秒），如果未知则为 null */
  duration: number | null
  /** 播放进度百分比 (0.0 - 1.0)，如果时长未知则为 null */
  progress: number | null
}

/** 服务状态 */
interface TauriServiceStatus {
  status: string
  state: string
  port: number
  error: null
}

/** 播放模式 */
export type PlayMode = 'sequential' | 'loop' | 'single_loop' | 'shuffle'

/** 服务器配置 */
export interface ServerConfig {
  bind_address: string
  port: number
  max_connections: number
}

// ===== Tauri 命令客户端 =====

class TauriCommandClient {
  // ===== 状态查询 =====

  /** 获取音频状态 */
  async getAudioState(): Promise<AudioState> {
    return await invoke<AudioState>('get_audio_state')
  }

  /** 获取服务状态 */
  async getServiceStatus(): Promise<TauriServiceStatus> {
    return await invoke<TauriServiceStatus>('get_service_status')
  }

  /** 获取播放进度 */
  async getPlaybackProgress(): Promise<PlaybackProgress> {
    return await invoke<PlaybackProgress>('get_playback_progress')
  }

  // ===== 播放控制 =====

  /** 播放音乐 */
  async play(): Promise<void> {
    await invoke('play_music')
  }

  /** 暂停音乐 */
  async pause(): Promise<void> {
    await invoke('pause_music')
  }

  /** 停止音乐 */
  async stop(): Promise<void> {
    await invoke('stop_music')
  }

  /** 下一首 */
  async next(): Promise<void> {
    await invoke('next_track')
  }

  /** 上一首 */
  async previous(): Promise<void> {
    await invoke('previous_track')
  }

  /** 播放指定索引的曲目 */
  async playTrackAt(index: number): Promise<void> {
    await invoke('play_track_at', { index })
  }

  // ===== 音量控制 =====

  /** 设置主音量 (0.0 - 1.0) */
  async setMasterVolume(volume: number): Promise<void> {
    await invoke('set_master_volume', { volume })
  }

  /** 设置音乐音量 (0.0 - 1.0) */
  async setMusicVolume(volume: number): Promise<void> {
    await invoke('set_music_volume', { volume })
  }

  /** 设置广播音量 (0.0 - 1.0) */
  async setBroadcastVolume(volume: number): Promise<void> {
    await invoke('set_broadcast_volume', { volume })
  }

  // ===== 音频闪避 =====

  /** 开始音频闪避 */
  async startDucking(): Promise<void> {
    await invoke('start_ducking')
  }

  /** 停止音频闪避 */
  async stopDucking(): Promise<void> {
    await invoke('stop_ducking')
  }

  /** 设置闪避功能是否启用 */
  async setDuckingEnabled(enabled: boolean): Promise<void> {
    await invoke('set_ducking_enabled', { enabled })
  }

  /** 设置闪避音量 (0.0 - 1.0) */
  async setDuckingVolume(volume: number): Promise<void> {
    await invoke('set_ducking_volume', { volume })
  }

  /** 设置闪避淡入时间（毫秒） */
  async setDuckingFadeInMs(ms: number): Promise<void> {
    await invoke('set_ducking_fade_in_ms', { ms })
  }

  /** 设置闪避淡出时间（毫秒） */
  async setDuckingFadeOutMs(ms: number): Promise<void> {
    await invoke('set_ducking_fade_out_ms', { ms })
  }

  // ===== 播放列表管理 =====

  /** 获取播放列表 */
  async getPlaylist(): Promise<PlaylistItem[]> {
    return await invoke<PlaylistItem[]>('get_playlist')
  }

  /** 添加曲目 */
  async addTrack(path: string): Promise<void> {
    await invoke('add_track', { path })
  }

  /** 移除曲目 */
  async removeTrack(index: number): Promise<void> {
    await invoke('remove_track', { index })
  }

  /** 清空播放列表 */
  async clearPlaylist(): Promise<void> {
    await invoke('clear_playlist')
  }

  /** 设置播放模式 */
  async setPlayMode(mode: PlayMode): Promise<void> {
    await invoke('set_play_mode', { mode })
  }

  /** 扫描目录 */
  async scanDirectory(path: string): Promise<number> {
    return await invoke<number>('scan_directory', { path })
  }

  // ===== 服务器配置 =====

  /** 获取服务器配置 */
  async getServerConfig(): Promise<ServerConfig> {
    return await invoke<ServerConfig>('get_server_config')
  }

  /** 更新服务器配置 */
  async updateServerConfig(config: ServerConfig): Promise<void> {
    await invoke('update_server_config', { configDto: config })
  }

  /** 重置所有配置为默认值 */
  async resetAllConfig(): Promise<void> {
    await invoke('reset_all_config')
  }

  // ===== 开机自启动 =====

  /** 启用开机自启动 */
  async enableAutostart(): Promise<void> {
    await invoke('enable_autostart')
  }

  /** 禁用开机自启动 */
  async disableAutostart(): Promise<void> {
    await invoke('disable_autostart')
  }

  /** 检查开机自启动状态 */
  async isAutostartEnabled(): Promise<boolean> {
    return await invoke<boolean>('is_autostart_enabled')
  }
}

// 导出单例
export const tauriClient = new TauriCommandClient()
