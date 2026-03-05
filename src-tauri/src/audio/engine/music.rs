//! 背景音乐引擎
//!
//! 支持样本读取供混音器使用：
//! - 播放列表管理
//! - 多种播放模式（顺序、循环、随机）
//! - 音频闪避（广播时自动降低音量）
//! - 样本读取接口（供 AudioMixer 使用）

use parking_lot::{Mutex, RwLock};
use rodio::{Decoder, Source};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;

use super::super::metadata;
use super::super::mixer::DuckState;
use super::super::playlist::{PlayMode, Playlist, PlaylistItem};
use crate::error::{AudioError, AudioResult};

/// 目标采样率（与广播引擎一致）
const TARGET_SAMPLE_RATE: u32 = 48000;
/// 目标声道数
const TARGET_CHANNELS: u16 = 2;

/// 音乐播放状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MusicState {
    Stopped,
    Playing,
    Paused,
}

/// 闪避配置
#[derive(Debug, Clone)]
pub struct DuckingConfig {
    /// 是否启用闪避
    pub enabled: bool,
    /// 闪避目标音量
    pub volume: f32,
    /// 淡入时间（毫秒）
    pub fade_in_ms: u64,
    /// 淡出时间（毫秒）
    pub fade_out_ms: u64,
}

impl Default for DuckingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.3,
            fade_in_ms: 400,
            fade_out_ms: 600,
        }
    }
}

/// 音频解码器包装
struct AudioDecoder {
    /// 解码器
    decoder: Decoder<BufReader<File>>,
    /// 原始采样率
    source_sample_rate: u32,
    /// 原始声道数
    source_channels: u16,
    /// 是否已结束
    finished: bool,
}

impl AudioDecoder {
    /// 从文件创建解码器
    fn from_file(path: &PathBuf) -> AudioResult<Self> {
        let file = File::open(path).map_err(|e| {
            AudioError::FileNotFound(format!("无法打开文件 {}: {}", path.display(), e))
        })?;

        let reader = BufReader::new(file);
        let decoder = Decoder::new(reader)
            .map_err(|e| AudioError::Decode(format!("无法解码文件: {}", e)))?;

        // 获取源音频参数
        let source_sample_rate = decoder.sample_rate();
        let source_channels = decoder.channels();

        Ok(Self {
            decoder,
            source_sample_rate,
            source_channels,
            finished: false,
        })
    }

    /// 读取并重采样到目标格式
    fn read_samples(&mut self, output: &mut [f32]) -> usize {
        if self.finished {
            return 0;
        }

        let target_samples = output.len() / TARGET_CHANNELS as usize;
        let mut samples_read = 0;

        // 需要读取的源样本数（考虑采样率转换）
        let source_samples_needed = if self.source_sample_rate != TARGET_SAMPLE_RATE {
            // 简单的线性插值重采样
            (target_samples as f64 * self.source_sample_rate as f64 / TARGET_SAMPLE_RATE as f64).ceil() as usize
        } else {
            target_samples
        };

        // 临时缓冲区存储源样本
        let mut source_buffer = vec![0.0f32; source_samples_needed * self.source_channels as usize];
        let mut source_idx = 0;

        // 从解码器读取样本
        for sample in source_buffer.iter_mut() {
            match self.decoder.next() {
                Some(s) => {
                    // 将 i16 样本正确归一化到 [-1.0, 1.0] 范围
                    *sample = (s as f32) / i16::MAX as f32;
                    source_idx += 1;
                }
                None => {
                    self.finished = true;
                    break;
                }
            }
        }

        if source_idx == 0 {
            self.finished = true;
            return 0;
        }

        // 声道转换和重采样
        if self.source_channels == TARGET_CHANNELS && self.source_sample_rate == TARGET_SAMPLE_RATE {
            // 直接复制
            let copy_len = source_idx.min(output.len());
            output[..copy_len].copy_from_slice(&source_buffer[..copy_len]);
            samples_read = copy_len / TARGET_CHANNELS as usize;
        } else if self.source_channels == 1 && TARGET_CHANNELS == 2 {
            // 单声道转立体声 + 可能的重采样
            let mono_samples = source_idx;
            for i in 0..target_samples {
                let src_idx = if self.source_sample_rate != TARGET_SAMPLE_RATE {
                    ((i as f64 * self.source_sample_rate as f64 / TARGET_SAMPLE_RATE as f64) as usize).min(mono_samples - 1)
                } else {
                    i.min(mono_samples - 1)
                };

                let sample = source_buffer[src_idx];
                let out_idx = i * 2;
                if out_idx + 1 < output.len() {
                    output[out_idx] = sample;     // 左声道
                    output[out_idx + 1] = sample; // 右声道
                    samples_read += 1;
                }
            }
        } else if self.source_channels == 2 && TARGET_CHANNELS == 2 {
            // 立体声 + 重采样
            for i in 0..target_samples {
                let src_idx = if self.source_sample_rate != TARGET_SAMPLE_RATE {
                    ((i as f64 * self.source_sample_rate as f64 / TARGET_SAMPLE_RATE as f64) as usize).min(source_idx / 2 - 1)
                } else {
                    i
                };

                let out_idx = i * 2;
                let src_sample_idx = src_idx * 2;
                if out_idx + 1 < output.len() && src_sample_idx + 1 < source_idx {
                    output[out_idx] = source_buffer[src_sample_idx];
                    output[out_idx + 1] = source_buffer[src_sample_idx + 1];
                    samples_read += 1;
                }
            }
        } else {
            // 其他情况：简单填充
            for i in 0..target_samples.min(source_idx / self.source_channels as usize) {
                let out_idx = i * TARGET_CHANNELS as usize;
                if out_idx + 1 < output.len() {
                    output[out_idx] = source_buffer[i * self.source_channels as usize % source_idx];
                    output[out_idx + 1] = source_buffer[(i * self.source_channels as usize + 1) % source_idx];
                    samples_read += 1;
                }
            }
        }

        samples_read * TARGET_CHANNELS as usize
    }

    /// 是否已结束
    fn is_finished(&self) -> bool {
        self.finished
    }

    /// 获取音频总时长（样本数）
    ///
    /// 注意：rodio 解码器不直接提供总时长，这里返回 None
    /// 实际时长应该从播放列表项的元数据中获取
    fn total_duration(&mut self) -> Option<u64> {
        // rodio Decoder 不提供总时长信息
        // 需要使用其他库（如 symphonia）读取元数据
        // 或者在扫描时预解析并存储在播放列表中
        None
    }
}

/// 内部状态
struct MusicEngineInner {
    /// 当前解码器
    decoder: Option<AudioDecoder>,
    /// 样本缓冲区
    sample_buffer: VecDeque<f32>,
    /// 播放状态
    state: MusicState,
    /// 音量
    volume: f32,
    /// 当前曲目索引
    #[allow(dead_code)]
    current_index: usize,
    /// 音频闪避器
    ducker: crate::audio::mixer::AudioDucker,
    /// 闪避配置
    ducking_config: DuckingConfig,
    /// 当前播放位置（样本数）
    position_samples: u64,
    /// 音频总样本数（如果已知）
    total_samples: Option<u64>,
}

/// 背景音乐引擎
///
/// 提供样本读取接口，供 AudioMixer 进行混音。
pub struct MusicEngine {
    /// 播放列表
    playlist: Arc<Playlist>,
    /// 内部状态（线程安全）
    inner: Mutex<MusicEngineInner>,
    /// 音乐目录（用于扫描）
    #[allow(dead_code)]
    music_dir: RwLock<PathBuf>,
    /// 当前曲目 ID（用于状态查询）
    current_track_id: RwLock<Option<String>>,
    /// 当前曲目标题
    current_track_title: RwLock<Option<String>>,
    /// 闪避状态（用于状态查询）
    duck_state: RwLock<DuckState>,
}

impl MusicEngine {
    /// 创建新的音乐引擎
    #[allow(dead_code)]
    pub fn new(music_dir: PathBuf, volume: f32) -> Self {
        Self::with_ducking_and_playlist(music_dir, volume, DuckingConfig::default(), None)
    }

    /// 创建带有闪避配置的音乐引擎
    #[allow(dead_code)]
    pub fn with_ducking(music_dir: PathBuf, volume: f32, ducking_config: DuckingConfig) -> Self {
        Self::with_ducking_and_playlist(music_dir, volume, ducking_config, None)
    }

    /// 创建带有闪避配置和共享播放列表的音乐引擎
    pub fn with_ducking_and_playlist(
        music_dir: PathBuf,
        volume: f32,
        ducking_config: DuckingConfig,
        shared_playlist: Option<Arc<Playlist>>,
    ) -> Self {
        let ducker = crate::audio::mixer::AudioDucker::new(
            ducking_config.volume,
            ducking_config.fade_in_ms,
            ducking_config.fade_out_ms,
        );

        let inner = MusicEngineInner {
            decoder: None,
            sample_buffer: VecDeque::with_capacity(TARGET_SAMPLE_RATE as usize * 2), // 约2秒缓冲
            state: MusicState::Stopped,
            volume,
            current_index: 0,
            ducker,
            ducking_config,
            position_samples: 0,
            total_samples: None,
        };

        Self {
            playlist: shared_playlist.unwrap_or_else(|| Arc::new(Playlist::new())),
            inner: Mutex::new(inner),
            music_dir: RwLock::new(music_dir),
            current_track_id: RwLock::new(None),
            current_track_title: RwLock::new(None),
            duck_state: RwLock::new(DuckState::Normal),
        }
    }

    /// 读取音频样本（供混音器调用）
    ///
    /// # 参数
    /// - `output`: 输出缓冲区（交错格式，立体声）
    ///
    /// # 返回
    /// 实际读取的样本数（每声道）
    #[allow(clippy::needless_range_loop)]
    pub fn read_samples(&self, output: &mut [f32]) -> usize {
        let mut inner = self.inner.lock();

        // 只在真正闪避时才更新和使用闪避器（检查闪避状态）
        let duck_factor = match *self.duck_state.read() {
            DuckState::Normal => 1.0,
            _ => {
                // 正在闪避或过渡中，更新并使用闪避器
                if inner.ducking_config.enabled {
                    inner.ducker.update();
                    inner.ducker.volume()
                } else {
                    1.0
                }
            }
        };

        let effective_volume = inner.volume * duck_factor;

        // 如果暂停或停止，返回静音
        if inner.state != MusicState::Playing {
            for sample in output.iter_mut() {
                *sample = 0.0;
            }
            return output.len() / TARGET_CHANNELS as usize;
        }

        // 先从内部缓冲区读取
        let mut samples_written = 0;
        while samples_written < output.len() && !inner.sample_buffer.is_empty() {
            output[samples_written] = inner.sample_buffer.pop_front().unwrap();
            samples_written += 1;
        }

        // 如果缓冲区不足，从解码器读取更多
        while samples_written < output.len() {
            // 检查是否需要加载下一首
            let needs_next = inner.decoder.is_none() || inner.decoder.as_ref().is_none_or(|d| d.is_finished());

            if needs_next {
                // 尝试加载下一首
                let has_next = self.load_next_track_internal(&mut inner);
                if !has_next {
                    // 没有更多曲目，填充静音
                    while samples_written < output.len() {
                        output[samples_written] = 0.0;
                        samples_written += 1;
                    }
                    break;
                }
                continue; // 重新检查解码器状态
            }

            // 从解码器读取到临时缓冲区
            let remaining = output.len() - samples_written;
            let mut temp_buffer = vec![0.0f32; remaining];
            let mut read = 0usize;
            let mut decoder_finished = false;

            if let Some(ref mut decoder) = inner.decoder {
                read = decoder.read_samples(&mut temp_buffer);
                decoder_finished = decoder.is_finished();
            }
            // decoder 借用结束

            if read > 0 {
                for i in 0..read {
                    if samples_written < output.len() {
                        output[samples_written] = temp_buffer[i] * effective_volume;
                        samples_written += 1;
                    } else {
                        // 缓冲区已满，存入内部缓冲区
                        inner.sample_buffer.push_back(temp_buffer[i] * effective_volume);
                    }
                }
            }

            // 检查是否播放完成
            if decoder_finished {
                // 发送曲目结束事件
                if let Some(ref id) = *self.current_track_id.read() {
                    tracing::info!("播放结束: {}", id);
                }

                // 标记需要加载下一首，下次循环处理
                inner.decoder = None;
            }

            // 如果没有读到任何数据，退出循环
            if read == 0 {
                break;
            }
        }

        // 计算实际输出的样本数（每声道）
        let samples_per_channel = samples_written / TARGET_CHANNELS as usize;
        // 更新播放位置
        inner.position_samples += samples_per_channel as u64;

        samples_per_channel
    }

    /// 加载下一首曲目（内部方法）
    fn load_next_track_internal(&self, inner: &mut MusicEngineInner) -> bool {
        let items = self.playlist.items();
        if items.is_empty() {
            return false;
        }

        // 获取下一首
        self.playlist.next();
        let current = self.playlist.current();

        if let Some(item) = current {
            tracing::info!("正在加载曲目: {} 路径: {}", item.title, item.path.display());

            // 尝试获取音频总时长（使用 symphonia 读取元数据）
            let total_duration_secs = metadata::get_audio_duration(&item.path);
            tracing::info!("获取到的音频时长: {:?}", total_duration_secs);

            let total_duration_samples = total_duration_secs
                .map(|secs| (secs * TARGET_SAMPLE_RATE as f64) as u64);

            match AudioDecoder::from_file(&item.path) {
                Ok(mut decoder) => {
                    // 使用元数据中的时长，如果获取不到则尝试从解码器获取
                    inner.total_samples = total_duration_samples.or_else(|| decoder.total_duration());
                    // 重置播放位置
                    inner.position_samples = 0;
                    inner.decoder = Some(decoder);
                    *self.current_track_id.write() = Some(item.id.clone());
                    *self.current_track_title.write() = Some(item.title.clone());

                    if let Some(total) = inner.total_samples {
                        let duration_secs = total as f64 / TARGET_SAMPLE_RATE as f64;
                        tracing::info!("开始播放: {} ({}) - 时长: {:.2}秒", item.title, item.id, duration_secs);
                    } else {
                        tracing::info!("开始播放: {} ({})", item.title, item.id);
                    }
                    true
                }
                Err(e) => {
                    tracing::error!("加载曲目失败: {}", e);
                    // 尝试下一首
                    self.load_next_track_internal(inner)
                }
            }
        } else {
            false
        }
    }

    // ===== 公共 API =====

    /// 播放当前曲目
    pub fn play(&self) -> AudioResult<()> {
        let mut inner = self.inner.lock();

        if self.playlist.is_empty() {
            return Err(AudioError::Playback("播放列表为空".to_string()));
        }

        if inner.state == MusicState::Paused {
            // 恢复播放
            inner.state = MusicState::Playing;
            return Ok(());
        }

        // 开始新播放
        if inner.decoder.is_none() {
            if let Some(item) = self.playlist.current() {
                tracing::info!("正在加载曲目: {} 路径: {}", item.title, item.path.display());

                // 尝试获取音频总时长
                let total_duration_secs = metadata::get_audio_duration(&item.path);
                let total_duration_samples = total_duration_secs
                    .map(|secs| (secs * TARGET_SAMPLE_RATE as f64) as u64);

                match AudioDecoder::from_file(&item.path) {
                    Ok(mut decoder) => {
                        inner.total_samples = total_duration_samples.or_else(|| decoder.total_duration());
                        inner.position_samples = 0;
                        inner.decoder = Some(decoder);
                        *self.current_track_id.write() = Some(item.id.clone());
                        *self.current_track_title.write() = Some(item.title.clone());

                        if let Some(total) = inner.total_samples {
                            let duration_secs = total as f64 / TARGET_SAMPLE_RATE as f64;
                            tracing::info!("开始播放: {} - 时长: {:.2}秒", item.title, duration_secs);
                        } else {
                            tracing::info!("开始播放: {}", item.title);
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        }

        inner.state = MusicState::Playing;
        Ok(())
    }

    /// 播放指定曲目
    pub fn play_track(&self, id: &str) -> AudioResult<()> {
        if self.playlist.set_current(id) {
            let mut inner = self.inner.lock();
            inner.decoder = None; // 清除当前解码器

            if let Some(item) = self.playlist.current() {
                tracing::info!("正在加载曲目: {} 路径: {}", item.title, item.path.display());

                // 尝试获取音频总时长
                let total_duration_secs = metadata::get_audio_duration(&item.path);
                let total_duration_samples = total_duration_secs
                    .map(|secs| (secs * TARGET_SAMPLE_RATE as f64) as u64);

                match AudioDecoder::from_file(&item.path) {
                    Ok(mut decoder) => {
                        inner.total_samples = total_duration_samples.or_else(|| decoder.total_duration());
                        inner.position_samples = 0; // 重置播放位置
                        inner.decoder = Some(decoder);
                        inner.state = MusicState::Playing;
                        *self.current_track_id.write() = Some(item.id.clone());
                        *self.current_track_title.write() = Some(item.title.clone());

                        if let Some(total) = inner.total_samples {
                            let duration_secs = total as f64 / TARGET_SAMPLE_RATE as f64;
                            tracing::info!("开始播放: {} - 时长: {:.2}秒", item.title, duration_secs);
                        } else {
                            tracing::info!("开始播放: {}", item.title);
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }

    /// 暂停播放
    pub fn pause(&self) {
        self.inner.lock().state = MusicState::Paused;
    }

    /// 停止播放
    pub fn stop(&self) {
        let mut inner = self.inner.lock();
        inner.state = MusicState::Stopped;
        inner.decoder = None;
        inner.sample_buffer.clear();
        *self.current_track_id.write() = None;
        *self.current_track_title.write() = None;
    }

    /// 下一首
    pub fn next(&self) -> AudioResult<()> {
        let mut inner = self.inner.lock();
        self.playlist.next();

        if let Some(item) = self.playlist.current() {
            tracing::info!("正在加载曲目: {} 路径: {}", item.title, item.path.display());

            // 尝试获取音频总时长
            let total_duration_secs = metadata::get_audio_duration(&item.path);
            let total_duration_samples = total_duration_secs
                .map(|secs| (secs * TARGET_SAMPLE_RATE as f64) as u64);

            match AudioDecoder::from_file(&item.path) {
                Ok(mut decoder) => {
                    inner.total_samples = total_duration_samples.or_else(|| decoder.total_duration());
                    inner.position_samples = 0; // 重置播放位置
                    inner.decoder = Some(decoder);
                    inner.state = MusicState::Playing; // 确保状态为播放
                    *self.current_track_id.write() = Some(item.id.clone());
                    *self.current_track_title.write() = Some(item.title.clone());

                    if let Some(total) = inner.total_samples {
                        let duration_secs = total as f64 / TARGET_SAMPLE_RATE as f64;
                        tracing::info!("开始播放: {} - 时长: {:.2}秒", item.title, duration_secs);
                    } else {
                        tracing::info!("开始播放: {}", item.title);
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        } else {
            inner.decoder = None;
            inner.state = MusicState::Stopped;
        }

        Ok(())
    }

    /// 上一首
    pub fn previous(&self) -> AudioResult<()> {
        let mut inner = self.inner.lock();
        self.playlist.previous();

        if let Some(item) = self.playlist.current() {
            tracing::info!("正在加载曲目: {} 路径: {}", item.title, item.path.display());

            // 尝试获取音频总时长
            let total_duration_secs = metadata::get_audio_duration(&item.path);
            let total_duration_samples = total_duration_secs
                .map(|secs| (secs * TARGET_SAMPLE_RATE as f64) as u64);

            match AudioDecoder::from_file(&item.path) {
                Ok(mut decoder) => {
                    inner.total_samples = total_duration_samples.or_else(|| decoder.total_duration());
                    inner.position_samples = 0; // 重置播放位置
                    inner.decoder = Some(decoder);
                    inner.state = MusicState::Playing; // 确保状态为播放
                    *self.current_track_id.write() = Some(item.id.clone());
                    *self.current_track_title.write() = Some(item.title.clone());

                    if let Some(total) = inner.total_samples {
                        let duration_secs = total as f64 / TARGET_SAMPLE_RATE as f64;
                        tracing::info!("开始播放: {} - 时长: {:.2}秒", item.title, duration_secs);
                    } else {
                        tracing::info!("开始播放: {}", item.title);
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        } else {
            inner.decoder = None;
            inner.state = MusicState::Stopped;
        }

        Ok(())
    }

    /// 设置音量
    pub fn set_volume(&self, volume: f32) {
        self.inner.lock().volume = volume.clamp(0.0, 1.0);
    }

    /// 获取音量
    #[allow(dead_code)]
    pub fn volume(&self) -> f32 {
        self.inner.lock().volume
    }

    /// 获取播放状态
    #[allow(dead_code)]
    pub fn state(&self) -> MusicState {
        self.inner.lock().state
    }

    /// 是否正在播放
    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        self.inner.lock().state == MusicState::Playing
    }

    /// 获取播放列表
    #[allow(dead_code)]
    pub fn playlist(&self) -> Arc<Playlist> {
        self.playlist.clone()
    }

    /// 添加音乐文件
    #[allow(dead_code)]
    pub fn add_file(&self, path: PathBuf) -> String {
        let item = PlaylistItem::from_path(path);
        let id = item.id.clone();
        self.playlist.add(item);
        tracing::info!("添加音乐: {}", id);
        id
    }

    /// 从目录扫描音乐文件
    #[allow(dead_code)]
    pub fn scan_directory(&self, dir: &PathBuf) -> AudioResult<usize> {
        use walkdir::WalkDir;

        let extensions = ["mp3", "wav", "flac", "m4a", "ogg"];
        let mut count = 0;

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if extensions.contains(&ext.to_lowercase().as_str()) {
                    let item = PlaylistItem::from_path(path.to_path_buf());
                    self.playlist.add(item);
                    count += 1;
                }
            }
        }

        tracing::info!("从 {} 扫描到 {} 个音乐文件", dir.display(), count);
        Ok(count)
    }

    /// 设置播放模式
    #[allow(dead_code)]
    pub fn set_play_mode(&self, mode: PlayMode) {
        self.playlist.set_play_mode(mode);
    }

    /// 开始音频闪避（广播开始时调用）
    pub fn start_ducking(&self) {
        self.inner.lock().ducker.trigger_duck();
        *self.duck_state.write() = DuckState::FadingOut;
        tracing::info!("音频闪避触发");
    }

    /// 停止音频闪避（广播结束时调用）
    pub fn stop_ducking(&self) {
        self.inner.lock().ducker.trigger_normal();
        *self.duck_state.write() = DuckState::FadingIn;
        tracing::info!("音频闪避恢复");
    }

    /// 获取当前闪避状态
    #[allow(dead_code)]
    pub fn duck_state(&self) -> DuckState {
        *self.duck_state.read()
    }

    /// 设置闪避音量
    pub fn set_ducking_volume(&self, volume: f32) {
        self.inner.lock().ducker.set_duck_volume(volume);
    }

    /// 设置闪避淡入时间（毫秒）
    pub fn set_ducking_fade_in_ms(&self, ms: u64) {
        let mut inner = self.inner.lock();
        let fade_out = inner.ducker.fade_out_ms();
        inner.ducker.set_fade_times(ms, fade_out);
    }

    /// 设置闪避淡出时间（毫秒）
    pub fn set_ducking_fade_out_ms(&self, ms: u64) {
        let mut inner = self.inner.lock();
        let fade_in = inner.ducker.fade_in_ms();
        inner.ducker.set_fade_times(fade_in, ms);
    }
}

impl Drop for MusicEngine {
    fn drop(&mut self) {
        tracing::info!("音乐引擎关闭");
    }
}

/// 音乐状态（用于监控）
#[derive(Debug, Clone, Serialize)]
pub struct MusicStatus {
    /// 播放状态
    pub state: String,
    /// 音量
    pub volume: f32,
    /// 当前曲目 ID
    pub current_track_id: Option<String>,
    /// 当前曲目标题
    pub current_track_title: Option<String>,
    /// 播放列表长度
    pub playlist_length: usize,
    /// 闪避状态
    pub duck_state: String,
}

impl MusicEngine {
    /// 获取音乐状态（用于监控）
    pub fn get_status(&self) -> MusicStatus {
        let inner = self.inner.lock();
        MusicStatus {
            state: match inner.state {
                MusicState::Stopped => "stopped".to_string(),
                MusicState::Playing => "playing".to_string(),
                MusicState::Paused => "paused".to_string(),
            },
            volume: inner.volume,
            current_track_id: self.current_track_id.read().clone(),
            current_track_title: self.current_track_title.read().clone(),
            playlist_length: self.playlist.len(),
            duck_state: format!("{:?}", *self.duck_state.read()),
        }
    }

    /// 获取播放进度（秒）
    pub fn position(&self) -> f64 {
        let inner = self.inner.lock();
        (inner.position_samples as f64) / (TARGET_SAMPLE_RATE as f64)
    }

    /// 获取当前曲目总时长（秒）
    ///
    /// 如果未知（需要从元数据获取），返回 None
    pub fn duration(&self) -> Option<f64> {
        let inner = self.inner.lock();
        inner.total_samples.map(|s| (s as f64) / (TARGET_SAMPLE_RATE as f64))
    }

    /// 获取播放进度百分比 (0.0 - 1.0)
    #[allow(dead_code)]
    pub fn progress(&self) -> Option<f64> {
        let duration = self.duration()?;
        let position = self.position();
        Some(position / duration)
    }

    /// 获取当前播放索引
    pub fn current_index(&self) -> usize {
        self.playlist.current_index()
    }
}
