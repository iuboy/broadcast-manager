//! 广播音频引擎（无锁版本）
//!
//! 接收来自客户端的音频数据并缓冲，支持 PCM 和 Opus 编解码格式。
//!
//! # 无锁设计
//! 广播引擎运行在 Audio Actor 线程中，所有访问都在同一线程，无需锁。
//! 跨线程通信通过 crossbeam-channel（无锁队列）实现。

use std::collections::VecDeque;

use crate::audio::decoder::opus::{OpusConfig, OpusDecoderEngine};
use crate::error::{AudioError, AudioResult};

/// 广播音频编解码格式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCodec {
    Pcm,
    Opus,
}

/// 广播音频引擎（无锁版本 - 必须在 Audio Actor 线程中使用）
///
/// # 重要
/// 此引擎**不是线程安全的**，必须在 Audio Actor 线程中访问。
/// 跨线程通信通过 crossbeam-channel 实现。
///
/// # 架构
///
/// 网络线程 --crossbeam-channel--> Actor 线程 --> 广播引擎 --> 音频线程
///                                      (无锁)
///
pub struct BroadcastEngine {
    /// 编解码格式
    codec: AudioCodec,
    /// 采样率
    sample_rate: u32,
    /// 声道数
    channels: u16,
    /// PCM 缓冲区
    pcm_buffer: VecDeque<f32>,
    /// 音量
    volume: f32,
    /// 是否正在播放
    #[allow(dead_code)]
    is_playing: bool,
    /// Opus 解码器（如果使用 Opus 编解码）
    opus_decoder: Option<OpusDecoderEngine>,
}

#[allow(dead_code)]
impl BroadcastEngine {
    /// 创建默认配置的广播引擎（PCM, 44100Hz, 1 声道）
    pub fn new_default() -> Self {
        Self::new(44100, 1, "pcm".to_string())
    }

    /// 创建新的广播引擎
    ///
    /// # 参数
    /// - `sample_rate`: 采样率（8000-48000）
    /// - `channels`: 声道数（1 或 2）
    /// - `codec`: 编解码格式（pcm/opus）
    pub fn new(sample_rate: u32, channels: u16, codec: String) -> Self {
        let audio_codec = match codec.to_lowercase().as_str() {
            "opus" => AudioCodec::Opus,
            _ => AudioCodec::Pcm,
        };

        // 尝试初始化 Opus 解码器
        let opus_decoder = if audio_codec == AudioCodec::Opus {
            match OpusDecoderEngine::new(OpusConfig::new(sample_rate, channels)) {
                Ok(decoder) => {
                    tracing::info!(
                        "Opus 解码器初始化成功 ({}Hz, {} 声道)",
                        sample_rate,
                        channels
                    );
                    Some(decoder)
                }
                Err(e) => {
                    tracing::error!("Opus 解码器初始化失败: {}，将回退到 PCM 模式", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            codec: audio_codec,
            sample_rate,
            channels,
            pcm_buffer: VecDeque::new(),
            volume: 1.0,
            is_playing: false,
            opus_decoder,
        }
    }

    /// 重新配置编解码参数
    ///
    /// # 参数
    /// - `codec`: 编解码格式（pcm/opus）
    /// - `sample_rate`: 采样率（8000-48000）
    /// - `channels`: 声道数（1 或 2）
    pub fn reconfigure(&mut self, codec: &str, sample_rate: u32, channels: u16) {
        let new_codec = match codec.to_lowercase().as_str() {
            "opus" => AudioCodec::Opus,
            _ => AudioCodec::Pcm,
        };

        // 检查是否需要重新配置
        if self.codec == new_codec && self.sample_rate == sample_rate && self.channels == channels {
            tracing::debug!("广播引擎配置未变化，执行快速恢复");
            // 快速恢复：保留引擎，只清空缓冲区
            self.clear();
            return;
        }

        // 清空缓冲区
        self.clear();

        // 更新配置
        self.codec = new_codec;
        self.sample_rate = sample_rate;
        self.channels = channels;

        // 重新初始化 Opus 解码器（如果需要）
        if new_codec == AudioCodec::Opus {
            match OpusDecoderEngine::new(OpusConfig::new(sample_rate, channels)) {
                Ok(decoder) => {
                    tracing::info!(
                        "Opus 解码器重新配置成功 ({}Hz, {} 声道)",
                        sample_rate,
                        channels
                    );
                    self.opus_decoder = Some(decoder);
                }
                Err(e) => {
                    tracing::error!("Opus 解码器重新配置失败: {}，将回退到 PCM 模式", e);
                    self.codec = AudioCodec::Pcm;
                    self.opus_decoder = None;
                }
            }
        } else {
            self.opus_decoder = None;
        }

        tracing::info!(
            "广播引擎已重新配置: {:?} {}Hz {} 声道",
            self.codec,
            sample_rate,
            channels
        );
    }

    /// 快速恢复：保留引擎配置，只清空缓冲区
    ///
    /// 用于客户端切换时快速恢复，避免音频残留。
    pub fn quick_resume(&mut self) {
        self.clear();
        tracing::debug!("广播引擎快速恢复：缓冲区已清空");
    }

    /// 接收音频数据
    ///
    /// 根据 codec 配置自动选择解码方式：
    /// - PCM: 直接解析 16-bit 小端序数据
    /// - Opus: 使用 Opus 解码器解码
    pub fn receive_pcm(&mut self, data: &[u8]) -> AudioResult<()> {
        match self.codec {
            AudioCodec::Pcm => {
                // 将 16-bit PCM 转换为 f32
                for chunk in data.chunks(2) {
                    if chunk.len() == 2 {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                        let normalized = (sample as f32 / i16::MAX as f32) * self.volume;
                        self.pcm_buffer.push_back(normalized);
                    }
                }
            }
            AudioCodec::Opus => {
                // 使用 Opus 解码器
                if let Some(ref mut decoder) = self.opus_decoder {
                    match decoder.decode(data) {
                        Ok(samples) => {
                            for sample in samples {
                                self.pcm_buffer.push_back(sample * self.volume);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Opus 解码失败: {}", e);
                            return Err(e);
                        }
                    }
                } else {
                    return Err(AudioError::Decode(
                        "Opus 解码器未初始化，请检查系统是否安装 libopus".to_string(),
                    ));
                }
            }
        }

        // 限制缓冲区大小，防止内存溢出
        const MAX_BUFFER_SIZE: usize = 48000 * 2; // 约 2 秒
        if self.pcm_buffer.len() > MAX_BUFFER_SIZE {
            let drain = self.pcm_buffer.len() - MAX_BUFFER_SIZE;
            for _ in 0..drain {
                self.pcm_buffer.pop_front();
            }
            tracing::warn!("广播音频缓冲区溢出，丢弃 {} 个样本", drain);
        }

        Ok(())
    }

    /// 读取音频样本
    #[allow(clippy::needless_range_loop)]
    pub fn read_samples(&mut self, output: &mut [f32]) -> usize {
        let samples_to_read = output.len().min(self.pcm_buffer.len());

        for i in 0..samples_to_read {
            if let Some(sample) = self.pcm_buffer.pop_front() {
                output[i] = sample;
            }
        }

        // 如果缓冲区不足，填充静音
        for i in samples_to_read..output.len() {
            output[i] = 0.0;
        }

        samples_to_read
    }

    /// 设置音量
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// 获取音量
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// 开始播放
    pub fn start(&mut self) {
        self.is_playing = true;
        tracing::info!("广播引擎开始播放");
    }

    /// 停止播放
    pub fn stop(&mut self) {
        self.is_playing = false;
        self.clear();
        tracing::info!("广播引擎停止播放");
    }

    /// 是否正在播放
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.pcm_buffer.clear();

        // 重置 Opus 解码器状态
        if let Some(ref mut decoder) = self.opus_decoder {
            if let Err(e) = decoder.reset() {
                tracing::warn!("重置 Opus 解码器失败: {}", e);
            }
        }
    }

    /// 获取缓冲区大小（样本数）
    pub fn buffer_size(&self) -> usize {
        self.pcm_buffer.len()
    }

    /// 获取编解码格式
    pub fn codec(&self) -> AudioCodec {
        self.codec
    }

    /// 获取采样率
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 获取声道数
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// 检查 Opus 解码器是否可用
    pub fn is_opus_available(&self) -> bool {
        self.codec == AudioCodec::Opus && self.opus_decoder.is_some()
    }

    /// 获取当前编解码格式名称
    pub fn codec_name(&self) -> &'static str {
        match self.codec {
            AudioCodec::Pcm => "PCM",
            AudioCodec::Opus => {
                if self.is_opus_available() {
                    "Opus"
                } else {
                    "Opus (不可用)"
                }
            }
        }
    }
}

/// 广播状态（用于监控）
#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct BroadcastStatus {
    /// 是否正在播放
    pub is_playing: bool,
    /// 编解码格式
    pub codec: String,
    /// 采样率
    pub sample_rate: u32,
    /// 声道数
    pub channels: u16,
    /// 缓冲区大小
    pub buffer_size: usize,
    /// 音量
    pub volume: f32,
}

#[allow(dead_code)]
impl BroadcastEngine {
    /// 获取广播状态（用于监控）
    pub fn get_status(&self) -> BroadcastStatus {
        BroadcastStatus {
            is_playing: self.is_playing(),
            codec: self.codec_name().to_string(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            buffer_size: self.buffer_size(),
            volume: self.volume,
        }
    }
}
