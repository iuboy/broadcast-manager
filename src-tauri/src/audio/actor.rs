//! 音频 Actor 模式实现
//!
//! 将音频子系统封装为独立的 Actor，通过消息传递进行通信。
//! 解决 cpal::Stream 非 Send 的问题，在专用线程中运行音频系统。

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::engine::{BroadcastConsumer, BroadcastEngineLockFree, DuckingConfig, MusicEngine};
use super::mixer::AudioMixer;
use super::playlist::{Playlist, PlaylistItem};
use crate::config::Config;
use ringbuf::traits::Split;

/// 全局广播音频缓冲区消费者（音频回调线程使用）
/// 使用 RwLock 实现安全的线程间共享
static BROADCAST_CONSUMER: parking_lot::RwLock<Option<Arc<BroadcastConsumer>>> =
    parking_lot::RwLock::new(None);

/// 全局广播采样率（用于重采样）
static BROADCAST_SAMPLE_RATE: parking_lot::RwLock<u32> = parking_lot::RwLock::new(44100);

/// 获取当前广播采样率（用于音频混音器重采样）
pub fn broadcast_sample_rate() -> u32 {
    *BROADCAST_SAMPLE_RATE.read()
}

/// 设置广播采样率
fn set_broadcast_sample_rate(sample_rate: u32) {
    *BROADCAST_SAMPLE_RATE.write() = sample_rate;
}

/// 获取全局广播音频消费者（用于音频混音器）
///
/// 返回 Arc 克隆，正确处理引用计数，确保内存安全。
pub fn broadcast_consumer() -> Arc<BroadcastConsumer> {
    let guard = BROADCAST_CONSUMER.read();
    match guard.as_ref() {
        Some(consumer) => consumer.clone(),
        None => {
            drop(guard);
            tracing::error!("广播消费者未初始化");
            let ringbuf = ringbuf::HeapRb::<f32>::new(1);
            let (_prod, consumer) = ringbuf.split();
            Arc::new(BroadcastConsumer {
                consumer: std::cell::UnsafeCell::new(consumer),
            })
        }
    }
}

/// 设置全局广播音频消费者
fn set_broadcast_consumer(consumer: Arc<BroadcastConsumer>) {
    let mut guard = BROADCAST_CONSUMER.write();
    *guard = Some(consumer);
}

/// 查询超时时间（毫秒）
const QUERY_TIMEOUT_MS: u64 = 1000;

/// 混音器控制状态（内部使用）
#[derive(Debug, Clone)]
pub struct MixerControl {
    /// 主音量
    pub master_volume: f32,
    /// 是否正在闪避
    pub is_ducking: bool,
}

impl Default for MixerControl {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            is_ducking: false,
        }
    }
}

/// 音频 Actor 消息类型
#[derive(Debug, Clone)]
pub enum AudioMessage {
    // === 播放控制 ===
    /// 播放背景音乐
    PlayMusic,
    /// 暂停背景音乐
    PauseMusic,
    /// 停止背景音乐
    StopMusic,
    /// 下一首
    NextTrack,
    /// 上一首
    PreviousTrack,
    /// 播放指定 ID 的曲目
    PlayTrackById {
        id: String,
        reply: Sender<Result<(), String>>,
    },

    // === 音量控制 ===
    /// 设置音乐音量 (0.0 - 1.0)
    SetMusicVolume(f32),
    /// 设置广播音量 (0.0 - 1.0)
    SetBroadcastVolume(f32),
    /// 设置主音量 (0.0 - 1.0)
    SetMasterVolume(f32),

    // === 音频闪避 ===
    /// 开始闪避（广播时降低音乐音量）
    StartDucking,
    /// 停止闪避（广播结束后恢复音乐音量）
    StopDucking,
    /// 设置闪避功能是否启用
    SetDuckingEnabled(bool),
    /// 设置闪避音量
    SetDuckingVolume(f32),
    /// 设置闪避淡入时间（毫秒）
    SetDuckingFadeInMs(u64),
    /// 设置闪避淡出时间（毫秒）
    SetDuckingFadeOutMs(u64),

    // === 播放列表 ===
    /// 添加曲目
    AddTrack { path: String, reply: Sender<()> },
    /// 移除曲目
    RemoveTrack { index: usize, reply: Sender<()> },
    /// 清空播放列表
    ClearPlaylist { reply: Sender<()> },
    /// 设置播放列表
    SetPlaylist { tracks: Vec<String> },
    /// 设置播放模式
    SetPlayMode { mode: String },
    /// 扫描目录
    ScanDirectory { path: String, reply: Sender<usize> },

    // === 广播数据 ===
    /// 推送广播音频数据 (PCM 16-bit)
    BroadcastData { samples: Vec<i16> },
    /// 推送广播音频数据 (PCM f32)
    BroadcastDataF32 { samples: Vec<f32> },
    /// 推送广播音频数据 (字节)
    BroadcastBytes { data: Vec<u8> },
    /// 重新配置广播引擎
    ReconfigureBroadcast {
        codec: String,
        sample_rate: u32,
        channels: u16,
    },

    // === 生命周期 ===
    /// 关闭 Actor
    Shutdown,

    // === 查询 ===
    /// 获取状态（带回复通道）
    GetState { reply: Sender<AudioState> },
    /// 获取播放列表路径
    GetPlaylist { reply: Sender<Vec<String>> },
    /// 获取播放列表详情
    GetPlaylistItems { reply: Sender<Vec<PlaylistItemDto>> },
    /// 获取播放进度
    GetPlaybackProgress { reply: Sender<PlaybackProgress> },
}

/// 音频 Actor 状态
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioState {
    /// 是否正在播放音乐
    pub is_playing: bool,
    /// 当前播放的曲目
    pub current_track: Option<String>,
    /// 音乐音量
    pub music_volume: f32,
    /// 广播音量
    pub broadcast_volume: f32,
    /// 主音量
    pub master_volume: f32,
    /// 是否正在闪避
    pub is_ducking: bool,
    /// 闪避是否启用
    pub ducking_enabled: bool,
    /// 闪避音量
    pub ducking_volume: f32,
    /// 闪避淡入时间（毫秒）
    pub ducking_fade_in_ms: u64,
    /// 闪避淡出时间（毫秒）
    pub ducking_fade_out_ms: u64,
    /// 播放列表长度
    pub playlist_length: usize,
    /// 当前播放索引
    pub current_index: usize,
    /// 播放模式
    pub play_mode: String,
}

/// 播放进度（用于 Tauri 命令返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackProgress {
    /// 当前播放位置（秒）
    pub position: f64,
    /// 音频总时长（秒），如果未知则为 None
    pub duration: Option<f64>,
    /// 播放进度百分比 (0.0 - 1.0)，如果时长未知则为 None
    pub progress: Option<f64>,
}

/// 播放列表项 DTO（用于 Tauri 命令返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItemDto {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// 时长（秒）
    pub duration: Option<u64>,
    pub path: Option<String>,
}

impl From<PlaylistItem> for PlaylistItemDto {
    fn from(item: PlaylistItem) -> Self {
        Self {
            id: item.id,
            title: item.title,
            artist: item.artist,
            album: item.album,
            duration: item.duration,
            path: Some(item.path.to_string_lossy().to_string()),
        }
    }
}

/// 音频 Actor 句柄（对外接口）
#[derive(Clone, Debug)]
pub struct AudioActor {
    /// 消息发送通道
    tx: Sender<AudioMessage>,
    /// 状态缓存（用于快速读取）
    state_cache: Arc<Mutex<AudioState>>,
}

impl AudioActor {
    /// 创建并启动音频 Actor
    pub fn spawn(config: &Config) -> Self {
        let (tx, rx) = unbounded();
        let state_cache = Arc::new(Mutex::new(AudioState::default()));
        let config = config.clone();

        // 启动 Actor 线程
        let state_clone = state_cache.clone();
        thread::spawn(move || {
            AudioActorCore::run(rx, state_clone, config);
        });

        AudioActor { tx, state_cache }
    }

    /// 发送消息（fire-and-forget）
    pub fn send(&self, msg: AudioMessage) {
        // 获取消息类型用于日志记录（在移动之前）
        let msg_type = std::mem::discriminant(&msg);
        if let Err(e) = self.tx.send(msg) {
            tracing::error!(
                "发送消息到音频 Actor 失败: 消息类型={:?}, 错误={:?}. \
                 这通常意味着音频 Actor 已崩溃或关闭。请重启应用。",
                msg_type,
                e
            );
        }
    }

    /// 获取缓存状态（快速，可能略有延迟）
    pub fn get_state(&self) -> AudioState {
        self.state_cache.lock().clone()
    }

    /// 查询状态（同步，确保最新）
    pub fn query_state(&self) -> AudioState {
        let (reply_tx, reply_rx) = unbounded();
        if let Err(e) = self.tx.send(AudioMessage::GetState { reply: reply_tx }) {
            tracing::error!("查询状态失败，无法发送请求: {:?}", e);
            return self.get_state(); // 回退到缓存状态
        }

        reply_rx
            .recv_timeout(Duration::from_millis(QUERY_TIMEOUT_MS))
            .unwrap_or_else(|e| {
                match e {
                    RecvTimeoutError::Timeout => {
                        tracing::warn!("查询状态超时，使用缓存状态");
                    }
                    RecvTimeoutError::Disconnected => {
                        tracing::error!("音频 Actor 已断开连接，使用缓存状态");
                    }
                }
                self.get_state()
            })
    }

    /// 查询播放列表
    pub fn query_playlist(&self) -> Vec<String> {
        let (reply_tx, reply_rx) = unbounded();
        if let Err(e) = self.tx.send(AudioMessage::GetPlaylist { reply: reply_tx }) {
            tracing::error!("查询播放列表失败，无法发送请求: {:?}", e);
            return Vec::new();
        }

        reply_rx
            .recv_timeout(Duration::from_millis(QUERY_TIMEOUT_MS))
            .unwrap_or_else(|e| {
                tracing::warn!("查询播放列表失败: {:?}", e);
                Vec::new()
            })
    }

    // === 便捷方法 ===

    /// 播放音乐
    pub fn play(&self) {
        self.send(AudioMessage::PlayMusic);
    }

    /// 暂停音乐
    pub fn pause(&self) {
        self.send(AudioMessage::PauseMusic);
    }

    /// 停止音乐
    pub fn stop(&self) {
        self.send(AudioMessage::StopMusic);
    }

    /// 下一首
    pub fn next(&self) {
        self.send(AudioMessage::NextTrack);
    }

    /// 上一首
    pub fn previous(&self) {
        self.send(AudioMessage::PreviousTrack);
    }

    /// 播放指定索引的曲目
    pub fn play_track_at(&self, index: usize) -> Result<(), String> {
        // 获取播放列表
        let items = self.get_playlist_items();

        if index >= items.len() {
            return Err("索引超出播放列表范围".to_string());
        }

        // 通过 ID 播放
        let id = &items[index].id;
        self.play_track_by_id(id)
    }

    /// 通过 ID 播放曲目
    pub fn play_track_by_id(&self, id: &str) -> Result<(), String> {
        let (reply_tx, reply_rx) = unbounded();
        if let Err(e) = self.tx.send(AudioMessage::PlayTrackById {
            id: id.to_string(),
            reply: reply_tx,
        }) {
            return Err(format!("无法发送播放命令，音频系统可能无响应: {}", e));
        }

        reply_rx
            .recv_timeout(Duration::from_millis(5000))
            .map_err(|e| format!("播放请求超时或音频系统已断开: {}", e))?
    }

    /// 设置音乐音量
    pub fn set_music_volume(&self, volume: f32) {
        self.send(AudioMessage::SetMusicVolume(volume));
    }

    /// 设置广播音量
    pub fn set_broadcast_volume(&self, volume: f32) {
        self.send(AudioMessage::SetBroadcastVolume(volume));
    }

    /// 设置主音量
    pub fn set_master_volume(&self, volume: f32) {
        self.send(AudioMessage::SetMasterVolume(volume));
    }

    /// 开始闪避
    pub fn start_ducking(&self) {
        self.send(AudioMessage::StartDucking);
    }

    /// 停止闪避
    pub fn stop_ducking(&self) {
        self.send(AudioMessage::StopDucking);
    }

    /// 设置闪避功能是否启用
    pub fn set_ducking_enabled(&self, enabled: bool) {
        self.send(AudioMessage::SetDuckingEnabled(enabled));
    }

    /// 设置闪避音量
    pub fn set_ducking_volume(&self, volume: f32) {
        self.send(AudioMessage::SetDuckingVolume(volume));
    }

    /// 设置闪避淡入时间
    pub fn set_ducking_fade_in_ms(&self, ms: u64) {
        self.send(AudioMessage::SetDuckingFadeInMs(ms));
    }

    /// 设置闪避淡出时间
    pub fn set_ducking_fade_out_ms(&self, ms: u64) {
        self.send(AudioMessage::SetDuckingFadeOutMs(ms));
    }

    /// 推送广播数据
    pub fn push_broadcast_data(&self, samples: Vec<i16>) {
        self.send(AudioMessage::BroadcastData { samples });
    }

    /// 推送广播数据 (f32)
    pub fn push_broadcast_data_f32(&self, samples: Vec<f32>) {
        self.send(AudioMessage::BroadcastDataF32 { samples });
    }

    /// 推送广播数据 (字节)
    pub fn push_broadcast_bytes(&self, data: Vec<u8>) {
        self.send(AudioMessage::BroadcastBytes { data });
    }

    /// 重新配置广播引擎
    pub fn reconfigure_broadcast(&self, codec: String, sample_rate: u32, channels: u16) {
        self.send(AudioMessage::ReconfigureBroadcast {
            codec,
            sample_rate,
            channels,
        });
    }

    /// 获取播放列表项
    pub fn get_playlist_items(&self) -> Vec<PlaylistItemDto> {
        let (reply_tx, reply_rx) = unbounded();
        if let Err(e) = self
            .tx
            .send(AudioMessage::GetPlaylistItems { reply: reply_tx })
        {
            tracing::error!("查询播放列表项失败，无法发送请求: {:?}", e);
            return Vec::new();
        }

        reply_rx
            .recv_timeout(Duration::from_millis(QUERY_TIMEOUT_MS))
            .unwrap_or_else(|e| {
                tracing::warn!("查询播放列表项失败: {:?}", e);
                Vec::new()
            })
    }

    /// 获取播放进度
    pub fn get_playback_progress(&self) -> PlaybackProgress {
        let (reply_tx, reply_rx) = unbounded();
        if let Err(e) = self
            .tx
            .send(AudioMessage::GetPlaybackProgress { reply: reply_tx })
        {
            tracing::error!("查询播放进度失败，无法发送请求: {:?}", e);
            return PlaybackProgress {
                position: 0.0,
                duration: None,
                progress: None,
            };
        }

        reply_rx
            .recv_timeout(Duration::from_millis(QUERY_TIMEOUT_MS))
            .unwrap_or_else(|e| {
                tracing::warn!("查询播放进度失败: {:?}", e);
                PlaybackProgress {
                    position: 0.0,
                    duration: None,
                    progress: None,
                }
            })
    }

    /// 设置播放模式
    pub fn set_play_mode(&self, mode: &str) -> Result<(), String> {
        match mode {
            "sequential" | "loop" | "single_loop" | "shuffle" => {
                self.send(AudioMessage::SetPlayMode {
                    mode: mode.to_string(),
                });
                Ok(())
            }
            _ => Err(format!("无效的播放模式: {}", mode)),
        }
    }

    /// 扫描目录
    pub fn scan_directory(&self, path: &str) -> Result<usize, String> {
        let (reply_tx, reply_rx) = unbounded();
        if let Err(e) = self.tx.send(AudioMessage::ScanDirectory {
            path: path.to_string(),
            reply: reply_tx,
        }) {
            return Err(format!("无法发送扫描命令，音频系统可能无响应: {}", e));
        }

        reply_rx
            .recv_timeout(Duration::from_secs(30)) // 扫描可能需要更长时间
            .map_err(|e| format!("扫描目录请求失败: {}", e))
    }

    /// 关闭 Actor
    pub fn shutdown(&self) {
        self.send(AudioMessage::Shutdown);
    }
}

/// 音频 Actor 核心 - 在专用线程中运行，持有所有音频资源
struct AudioActorCore {
    /// 音频混音器（持有 cpal::Stream）
    #[allow(dead_code)]
    mixer: AudioMixer,
    /// 音乐引擎
    music_engine: Arc<MusicEngine>,
    /// 广播引擎（无锁版本，仅在 Actor 线程访问）
    broadcast_engine: BroadcastEngineLockFree,
    /// 播放列表（共享）
    playlist: Arc<Playlist>,
    /// 当前状态
    state: AudioState,
    /// 混音器控制（共享）
    mixer_control: Arc<Mutex<MixerControl>>,
}

impl AudioActorCore {
    /// Actor 主循环
    fn run(rx: Receiver<AudioMessage>, state_cache: Arc<Mutex<AudioState>>, config: Config) {
        // 初始化音频系统
        let mut core = match Self::initialize(&config) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("音频 Actor 初始化失败: {}", e);
                return;
            }
        };

        // 初始化后立即同步状态到缓存
        core.update_cache(&state_cache);
        tracing::info!("音频 Actor 已启动，初始状态已同步");

        // 消息处理循环
        loop {
            match rx.recv() {
                Ok(msg) => {
                    if !core.handle_message(msg, &state_cache) {
                        break; // Shutdown
                    }
                }
                Err(_) => {
                    tracing::warn!("音频 Actor 通道已关闭");
                    break;
                }
            }
        }

        tracing::info!("音频 Actor 已关闭");
    }

    /// 初始化音频资源
    fn initialize(config: &Config) -> Result<Self, anyhow::Error> {
        // 创建混音器控制（使用 runtime 配置的主音量）
        let mixer_control = Arc::new(Mutex::new(MixerControl {
            master_volume: config.runtime.master_volume,
            is_ducking: false,
        }));

        // 创建共享的播放列表并从配置加载
        let playlist = Arc::new(Playlist::new());
        // 从配置加载播放列表
        if !config.playlist.is_empty() {
            tracing::info!("从配置加载 {} 个播放列表项", config.playlist.len());
            for path in &config.playlist {
                let item = PlaylistItem::from_path(PathBuf::from(path));
                playlist.add(item);
            }
        }

        // 创建无锁广播引擎（内部管理环形缓冲区）
        // 使用 create 方法同时创建引擎和消费者
        let (mut broadcast_engine, consumer) = BroadcastEngineLockFree::create(
            config.broadcast.sample_rate,
            config.broadcast.channels,
            config.broadcast.codec.clone(),
            500, // 500ms 缓冲区（减少溢出断续）
        );
        // 设置广播音量
        broadcast_engine.set_volume(config.runtime.broadcast_volume);

        // 设置全局消费者和采样率（供音频回调线程使用）
        set_broadcast_sample_rate(config.broadcast.sample_rate);
        set_broadcast_consumer(Arc::new(consumer));

        // 创建音乐引擎（使用 runtime 配置的音量和共享播放列表）
        let music_engine = Arc::new(MusicEngine::with_ducking_and_playlist(
            PathBuf::from(&config.music.music_dir),
            config.runtime.music_volume,
            DuckingConfig {
                enabled: config.runtime.ducking_enabled,
                volume: config.runtime.ducking_volume,
                fade_in_ms: config.runtime.ducking_fade_in_ms,
                fade_out_ms: config.runtime.ducking_fade_out_ms,
            },
            Some(playlist.clone()),
        ));

        // 创建混音器（这会创建 cpal::Stream，必须在当前线程）
        let mixer = AudioMixer::new(
            music_engine.clone(),
            mixer_control.clone(),
            config.runtime.ducking_volume,
            config.runtime.ducking_fade_in_ms,
            config.runtime.ducking_fade_out_ms,
        )?;

        let state = AudioState {
            is_playing: false,
            current_track: None,
            music_volume: config.runtime.music_volume,
            broadcast_volume: config.runtime.broadcast_volume,
            master_volume: config.runtime.master_volume,
            is_ducking: false,
            ducking_enabled: config.runtime.ducking_enabled,
            ducking_volume: config.runtime.ducking_volume,
            ducking_fade_in_ms: config.runtime.ducking_fade_in_ms,
            ducking_fade_out_ms: config.runtime.ducking_fade_out_ms,
            playlist_length: playlist.len(),
            current_index: 0,
            play_mode: "sequential".to_string(),
        };

        Ok(Self {
            mixer,
            music_engine,
            broadcast_engine,
            playlist,
            state,
            mixer_control,
        })
    }

    /// 处理消息，返回 false 表示退出
    fn handle_message(&mut self, msg: AudioMessage, cache: &Arc<Mutex<AudioState>>) -> bool {
        match msg {
            // === 播放控制 ===
            AudioMessage::PlayMusic => {
                if let Err(e) = self.music_engine.play() {
                    tracing::error!("播放失败: {}", e);
                } else {
                    self.state.is_playing = true;
                    // 更新当前曲目信息
                    let status = self.music_engine.get_status();
                    self.state.current_track = status.current_track_id;
                    self.state.current_index = self.music_engine.current_index();
                    self.update_cache(cache);
                }
            }
            AudioMessage::PauseMusic => {
                self.music_engine.pause();
                self.state.is_playing = false;
                self.update_cache(cache);
            }
            AudioMessage::StopMusic => {
                self.music_engine.stop();
                self.state.is_playing = false;
                self.update_cache(cache);
            }
            AudioMessage::NextTrack => {
                if let Err(e) = self.music_engine.next() {
                    tracing::error!("下一首失败: {}", e);
                } else {
                    self.state.is_playing = true;
                    // 更新当前曲目信息
                    let status = self.music_engine.get_status();
                    self.state.current_track = status.current_track_id;
                    self.state.current_index = self.music_engine.current_index();
                    self.update_cache(cache);
                }
            }
            AudioMessage::PreviousTrack => {
                if let Err(e) = self.music_engine.previous() {
                    tracing::error!("上一首失败: {}", e);
                } else {
                    self.state.is_playing = true;
                    // 更新当前曲目信息
                    let status = self.music_engine.get_status();
                    self.state.current_track = status.current_track_id;
                    self.state.current_index = self.music_engine.current_index();
                    self.update_cache(cache);
                }
            }
            AudioMessage::PlayTrackById { id, reply } => {
                let result = self.music_engine.play_track(&id);
                if result.is_ok() {
                    self.state.is_playing = true;
                    // 更新当前曲目信息
                    let status = self.music_engine.get_status();
                    self.state.current_track = status.current_track_id;
                    self.state.current_index = self.music_engine.current_index();
                    self.update_cache(cache);
                }
                if let Err(e) = reply.send(result.map_err(|e| e.to_string())) {
                    tracing::warn!("无法发送播放结果到查询端: {:?}", e);
                }
            }

            // === 音量控制 ===
            AudioMessage::SetMusicVolume(v) => {
                self.music_engine.set_volume(v);
                self.state.music_volume = v;
                self.update_cache(cache);
            }
            AudioMessage::SetBroadcastVolume(v) => {
                self.broadcast_engine.set_volume(v);
                self.state.broadcast_volume = v;
                self.update_cache(cache);
            }
            AudioMessage::SetMasterVolume(v) => {
                self.mixer_control.lock().master_volume = v;
                self.state.master_volume = v;
                self.update_cache(cache);
            }

            // === 音频闪避 ===
            AudioMessage::StartDucking => {
                self.music_engine.start_ducking();
                self.state.is_ducking = true;
                self.state.ducking_enabled = true;
                self.mixer_control.lock().is_ducking = true;
                self.update_cache(cache);
            }
            AudioMessage::StopDucking => {
                tracing::info!("Audio Actor 收到 StopDucking 消息");
                self.music_engine.stop_ducking();
                self.state.is_ducking = false;
                self.mixer_control.lock().is_ducking = false;
                tracing::info!(
                    "Audio Actor 已停止闪避，is_ducking = {}",
                    self.state.is_ducking
                );
                self.update_cache(cache);
            }
            AudioMessage::SetDuckingEnabled(enabled) => {
                self.state.ducking_enabled = enabled;
                if !enabled {
                    // 禁用时停止当前闪避
                    self.music_engine.stop_ducking();
                    self.state.is_ducking = false;
                    self.mixer_control.lock().is_ducking = false;
                }
                self.update_cache(cache);
            }
            AudioMessage::SetDuckingVolume(v) => {
                self.music_engine.set_ducking_volume(v);
                self.state.ducking_volume = v;
                self.update_cache(cache);
            }
            AudioMessage::SetDuckingFadeInMs(ms) => {
                self.music_engine.set_ducking_fade_in_ms(ms);
                self.state.ducking_fade_in_ms = ms;
                self.update_cache(cache);
            }
            AudioMessage::SetDuckingFadeOutMs(ms) => {
                self.music_engine.set_ducking_fade_out_ms(ms);
                self.state.ducking_fade_out_ms = ms;
                self.update_cache(cache);
            }

            // === 播放列表 ===
            AudioMessage::AddTrack { path, reply } => {
                let item = PlaylistItem::from_path(PathBuf::from(&path));
                self.playlist.add(item);
                self.state.playlist_length = self.playlist.len();
                self.update_cache(cache);
                let _ = reply.send(());
            }
            AudioMessage::RemoveTrack { index, reply } => {
                let items = self.playlist.items();
                if let Some(item) = items.get(index) {
                    self.playlist.remove(&item.id);
                    self.state.playlist_length = self.playlist.len();
                    self.update_cache(cache);
                }
                let _ = reply.send(());
            }
            AudioMessage::ClearPlaylist { reply } => {
                self.playlist.clear();
                self.state.playlist_length = 0;
                self.update_cache(cache);
                let _ = reply.send(());
            }
            AudioMessage::SetPlaylist { tracks } => {
                self.playlist.clear();
                for track in tracks {
                    let item = PlaylistItem::from_path(PathBuf::from(&track));
                    self.playlist.add(item);
                }
                self.state.playlist_length = self.playlist.len();
                self.update_cache(cache);
            }
            AudioMessage::SetPlayMode { mode } => {
                let play_mode = match mode.as_str() {
                    "sequential" => super::playlist::PlayMode::Sequential,
                    "loop" => super::playlist::PlayMode::Loop,
                    "single_loop" => super::playlist::PlayMode::SingleLoop,
                    "shuffle" => super::playlist::PlayMode::Shuffle,
                    _ => {
                        tracing::warn!("未知的播放模式: {}, 使用默认 Loop", mode);
                        super::playlist::PlayMode::Loop
                    }
                };
                self.playlist.set_play_mode(play_mode);
                self.state.play_mode = mode;
                self.update_cache(cache);
            }
            AudioMessage::ScanDirectory { path, reply } => {
                let count = match std::fs::read_dir(&path) {
                    Ok(entries) => {
                        let mut added = 0;
                        let mut skipped = 0;
                        for entry in entries {
                            let entry = match entry {
                                Ok(e) => e,
                                Err(e) => {
                                    tracing::warn!("跳过无法读取的目录项: {}", e);
                                    skipped += 1;
                                    continue;
                                }
                            };

                            if let Ok(file_type) = entry.file_type() {
                                if file_type.is_file() {
                                    if let Some(ext) = entry.path().extension() {
                                        if ext == "mp3"
                                            || ext == "wav"
                                            || ext == "flac"
                                            || ext == "ogg"
                                            || ext == "m4a"
                                        {
                                            let item = PlaylistItem::from_path(entry.path());
                                            self.playlist.add(item);
                                            added += 1;
                                        }
                                    }
                                }
                            }
                        }
                        self.state.playlist_length = self.playlist.len();
                        self.update_cache(cache);
                        if skipped > 0 {
                            tracing::info!(
                                "扫描目录完成: 添加 {} 个文件，跳过 {} 个",
                                added,
                                skipped
                            );
                        }
                        added
                    }
                    Err(e) => {
                        tracing::error!("扫描目录失败 {}: {}", path, e);
                        0
                    }
                };
                if let Err(e) = reply.send(count) {
                    tracing::warn!("无法发送扫描结果到查询端: {:?}", e);
                }
            }

            // === 广播数据 ===
            AudioMessage::BroadcastData { samples } => {
                // 将 i16 样本转换为字节
                let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
                if let Err(e) = self.broadcast_engine.receive_pcm(&bytes) {
                    tracing::warn!("广播数据接收失败: {}", e);
                }
            }
            AudioMessage::BroadcastDataF32 { samples } => {
                // 将 f32 样本转换为 i16 再转换为字节
                let bytes: Vec<u8> = samples
                    .iter()
                    .flat_map(|s| ((*s * i16::MAX as f32) as i16).to_le_bytes())
                    .collect();
                if let Err(e) = self.broadcast_engine.receive_pcm(&bytes) {
                    tracing::warn!("广播数据接收失败: {}", e);
                }
            }
            AudioMessage::BroadcastBytes { data } => {
                if let Err(e) = self.broadcast_engine.receive_pcm(&data) {
                    tracing::warn!("广播数据处理失败: {}", e);
                }
            }
            AudioMessage::ReconfigureBroadcast {
                codec,
                sample_rate,
                channels,
            } => {
                tracing::info!("重新配置广播: {}Hz", sample_rate);
                // 更新全局广播采样率（用于混音器重采样）
                set_broadcast_sample_rate(sample_rate);
                // 重新配置广播引擎并获取新的消费者
                let consumer =
                    self.broadcast_engine
                        .reconfigure(&codec, sample_rate, channels, 500);
                // 更新全局消费者（可以多次更新）
                set_broadcast_consumer(Arc::new(consumer));
            }

            // === 生命周期 ===
            AudioMessage::Shutdown => {
                return false;
            }

            // === 查询 ===
            AudioMessage::GetState { reply } => {
                if let Err(e) = reply.send(self.state.clone()) {
                    tracing::warn!("无法发送状态到查询端: {:?}", e);
                }
            }
            AudioMessage::GetPlaylist { reply } => {
                let tracks: Vec<String> = self
                    .playlist
                    .items()
                    .iter()
                    .map(|t| t.path.to_string_lossy().to_string())
                    .collect();
                if let Err(e) = reply.send(tracks) {
                    tracing::warn!("无法发送播放列表到查询端: {:?}", e);
                }
            }
            AudioMessage::GetPlaylistItems { reply } => {
                let items: Vec<PlaylistItemDto> = self
                    .playlist
                    .items()
                    .into_iter()
                    .map(PlaylistItemDto::from)
                    .collect();
                if let Err(e) = reply.send(items) {
                    tracing::warn!("无法发送播放列表项到查询端: {:?}", e);
                }
            }
            AudioMessage::GetPlaybackProgress { reply } => {
                let position = self.music_engine.position();
                let duration = self.music_engine.duration();
                let progress = duration.map(|d| position / d);
                if let Err(e) = reply.send(PlaybackProgress {
                    position,
                    duration,
                    progress,
                }) {
                    tracing::warn!("无法发送播放进度到查询端: {:?}", e);
                }
            }
        }
        true
    }

    /// 更新状态缓存
    fn update_cache(&self, cache: &Arc<Mutex<AudioState>>) {
        *cache.lock() = self.state.clone();
    }
}
