//! 音频混音器
//!
//! 负责将广播音频和背景音乐混合后输出到扬声器。
//! 支持：
//! - 多音源混音（广播 + 背景音乐）
//! - 音频闪避（广播时自动降低背景音乐音量）
//! - 独立音量控制
//! - 无锁环形缓冲区读取广播音频

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::ducker::AudioDucker;
use crate::audio::engine::{read_from_ringbuf, BroadcastConsumer, MusicEngine};
use crate::audio::{broadcast_consumer, MixerControl};
use crate::error::{AudioError, AudioResult};

/// 线性插值重采样器
///
/// 将音频从输入采样率转换到输出采样率。
/// 使用线性插值算法，简单快速，适合实时音频处理。
#[allow(clippy::needless_range_loop)]
fn resample_linear(input: &[f32], input_rate: u32, output_rate: u32, output: &mut [f32]) {
    if input_rate == output_rate {
        // 采样率相同，直接复制
        let len = input.len().min(output.len());
        output[..len].copy_from_slice(&input[..len]);
        // 填充剩余部分为静音
        for i in len..output.len() {
            output[i] = 0.0;
        }
        return;
    }

    let ratio = input_rate as f64 / output_rate as f64;

    for i in 0..output.len() {
        let pos = i as f64 * ratio;
        let index = pos as usize;
        let frac = pos - index as f64;

        if index + 1 < input.len() {
            // 线性插值
            output[i] = input[index] * (1.0 - frac) as f32 + input[index + 1] * frac as f32;
        } else if index < input.len() {
            // 超出范围，使用最后一个样本
            output[i] = input[index];
        } else {
            // 完全超出范围，填充静音
            output[i] = 0.0;
        }
    }
}

/// 共享混音状态（用于监控）
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MixerState {
    /// 主音量
    pub master_volume: f32,
    /// 背景音乐音量
    pub music_volume: f32,
    /// 广播音量
    pub broadcast_volume: f32,
    /// 闪避状态
    pub duck_state: String,
    /// 是否正在广播
    pub is_broadcasting: bool,
}

/// 音频混合器
///
/// 负责将广播音频和背景音乐混合后输出到扬声器。
/// 使用全局无锁环形缓冲区消费者读取广播音频。
pub struct AudioMixer {
    /// 音频输出流
    #[allow(dead_code)]
    stream: Option<Stream>,
    /// 音频输出流配置
    #[allow(dead_code)]
    config: StreamConfig,
    /// 混音器控制状态（与 HTTP API 共享）
    #[allow(dead_code)]
    mixer_control: Arc<Mutex<MixerControl>>,
    /// 音频闪避器
    #[allow(dead_code)]
    ducker: Arc<Mutex<AudioDucker>>,
    /// 音乐引擎引用
    #[allow(dead_code)]
    music_engine: Arc<MusicEngine>,
    /// 广播音频消费者（从全局静态获取）
    #[allow(dead_code)]
    broadcast_consumer: Arc<BroadcastConsumer>,
    /// 输出采样率（用于重采样）
    #[allow(dead_code)]
    output_sample_rate: u32,
}

impl AudioMixer {
    /// 创建新的音频混合器
    pub fn new(
        music_engine: Arc<MusicEngine>,
        mixer_control: Arc<Mutex<MixerControl>>,
        duck_volume: f32,
        fade_in_ms: u64,
        fade_out_ms: u64,
    ) -> AudioResult<Self> {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::DeviceInit("未找到默认音频输出设备".to_string()))?;

        let supported_config = device
            .default_output_config()
            .map_err(|e| AudioError::DeviceInit(format!("无法获取设备配置: {}", e)))?;

        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();
        let output_sample_rate = config.sample_rate.0; // 使用系统默认采样率

        tracing::info!(
            "音频输出设备: {}, 采样率: {} (系统默认), 声道: {}",
            device.name().unwrap_or_default(),
            output_sample_rate,
            config.channels
        );

        let ducker = Arc::new(Mutex::new(AudioDucker::new(
            duck_volume,
            fade_in_ms,
            fade_out_ms,
        )));

        // 获取全局广播音频消费者
        let broadcast_consumer = broadcast_consumer().clone();

        let err_fn = |err| tracing::error!("音频流错误: {}", err);

        // 根据采样格式创建流
        let output_channels = config.channels;
        let stream = match sample_format {
            SampleFormat::F32 => {
                let mixer_control_clone = mixer_control.clone();
                let music_clone = music_engine.clone();
                let ducker_clone = ducker.clone();
                let consumer_clone = broadcast_consumer.clone();
                device.build_output_stream::<f32, _, _>(
                    &config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::mix_audio_f32(
                            data,
                            &mixer_control_clone,
                            &consumer_clone,
                            &music_clone,
                            &ducker_clone,
                            output_sample_rate,
                            output_channels,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let mixer_control_clone = mixer_control.clone();
                let music_clone = music_engine.clone();
                let ducker_clone = ducker.clone();
                let consumer_clone = broadcast_consumer.clone();
                device.build_output_stream::<i16, _, _>(
                    &config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::mix_audio_i16(
                            data,
                            &mixer_control_clone,
                            &consumer_clone,
                            &music_clone,
                            &ducker_clone,
                            output_sample_rate,
                            output_channels,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let mixer_control_clone = mixer_control.clone();
                let music_clone = music_engine.clone();
                let ducker_clone = ducker.clone();
                let consumer_clone = broadcast_consumer.clone();
                device.build_output_stream::<u16, _, _>(
                    &config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::mix_audio_u16(
                            data,
                            &mixer_control_clone,
                            &consumer_clone,
                            &music_clone,
                            &ducker_clone,
                            output_sample_rate,
                            output_channels,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            format => {
                return Err(AudioError::DeviceInit(format!(
                    "不支持的采样格式: {:?}",
                    format
                )));
            }
        }
        .map_err(|e| AudioError::Playback(format!("无法创建音频流: {}", e)))?;

        stream
            .play()
            .map_err(|e| AudioError::Playback(format!("无法启动音频流: {}", e)))?;

        Ok(Self {
            stream: Some(stream),
            config,
            mixer_control,
            ducker,
            music_engine,
            broadcast_consumer,
            output_sample_rate,
        })
    }

    /// F32 格式混音
    #[allow(clippy::needless_range_loop)]
    fn mix_audio_f32(
        data: &mut [f32],
        mixer_control: &Arc<Mutex<MixerControl>>,
        _broadcast_consumer: &Arc<BroadcastConsumer>,
        music_engine: &Arc<MusicEngine>,
        ducker: &Arc<Mutex<AudioDucker>>,
        output_sample_rate: u32,
        output_channels: u16,
    ) {
        // 每帧更新闪避器状态
        let mut d = ducker.lock();
        d.update();
        drop(d);

        // 从共享控制状态获取主音量
        let master_vol = mixer_control.lock().master_volume;

        // 从全局静态获取最新的消费者（而不是使用参数传入的旧消费者）
        let current_consumer = crate::audio::broadcast_consumer();

        // 动态获取当前广播采样率
        let broadcast_sample_rate = crate::audio::broadcast_sample_rate();

        // 计算输出声道数和帧数
        let output_channels = output_channels as usize;
        let output_frames = data.len() / output_channels;

        // 广播数据是单声道，需要读取的样本数
        let broadcast_frames_needed = if broadcast_sample_rate == output_sample_rate {
            output_frames
        } else {
            ((output_frames as f64) * (broadcast_sample_rate as f64 / output_sample_rate as f64))
                .ceil() as usize
                + 100
        };

        // 从无锁环形缓冲区读取广播音频（单声道）
        let mut raw_broadcast_buffer = vec![0.0f32; broadcast_frames_needed];
        let actually_read = read_from_ringbuf(&current_consumer, &mut raw_broadcast_buffer);

        // 重采样到目标采样率（仍然是单声道）
        let mut broadcast_mono = vec![0.0f32; output_frames];
        if broadcast_sample_rate != output_sample_rate {
            resample_linear(
                &raw_broadcast_buffer,
                broadcast_sample_rate,
                output_sample_rate,
                &mut broadcast_mono,
            );
        } else {
            let copy_len = actually_read.min(output_frames);
            broadcast_mono[..copy_len].copy_from_slice(&raw_broadcast_buffer[..copy_len]);
        }

        // 将单声道广播数据转换为输出声道数（单声道 -> 立体声）
        let mut broadcast_buffer = vec![0.0f32; data.len()];
        for frame in 0..output_frames {
            let mono_sample = broadcast_mono[frame];
            for ch in 0..output_channels {
                broadcast_buffer[frame * output_channels + ch] = mono_sample;
            }
        }

        // 从音乐引擎读取样本（已应用引擎音量和闪避）
        let mut music_buffer = vec![0.0f32; data.len()];
        music_engine.read_samples(&mut music_buffer);

        // 混音（音乐引擎已经应用了闪避系数）
        for i in 0..data.len() {
            let broadcast = broadcast_buffer[i];
            let music = music_buffer[i];
            let mixed = broadcast + music;
            // 限制输出在 [-1.0, 1.0] 范围内，防止削波失真
            let sample = (mixed * master_vol).clamp(-1.0, 1.0);
            data[i] = sample;
        }
    }

    /// I16 格式混音
    #[allow(clippy::needless_range_loop)]
    fn mix_audio_i16(
        data: &mut [i16],
        mixer_control: &Arc<Mutex<MixerControl>>,
        _broadcast_consumer: &Arc<BroadcastConsumer>,
        music_engine: &Arc<MusicEngine>,
        ducker: &Arc<Mutex<AudioDucker>>,
        output_sample_rate: u32,
        output_channels: u16,
    ) {
        // 每帧更新闪避器状态
        let mut d = ducker.lock();
        d.update();
        drop(d);

        // 从共享控制状态获取主音量
        let master_vol = mixer_control.lock().master_volume;

        // 从全局静态获取最新的消费者
        let current_consumer = crate::audio::broadcast_consumer();

        // 动态获取当前广播采样率
        let broadcast_sample_rate = crate::audio::broadcast_sample_rate();

        // 计算输出声道数和帧数
        let output_channels = output_channels as usize;
        let output_frames = data.len() / output_channels;

        // 广播数据是单声道，需要读取的样本数（基于帧数）
        let broadcast_frames_needed = if broadcast_sample_rate == output_sample_rate {
            output_frames
        } else {
            ((output_frames as f64) * (broadcast_sample_rate as f64 / output_sample_rate as f64))
                .ceil() as usize
                + 100
        };

        // 从无锁环形缓冲区读取广播音频（单声道）
        let mut raw_broadcast_buffer = vec![0.0f32; broadcast_frames_needed];
        let actually_read = read_from_ringbuf(&current_consumer, &mut raw_broadcast_buffer);

        // 重采样到目标采样率（仍然是单声道）
        let mut broadcast_mono = vec![0.0f32; output_frames];
        if broadcast_sample_rate != output_sample_rate {
            resample_linear(
                &raw_broadcast_buffer,
                broadcast_sample_rate,
                output_sample_rate,
                &mut broadcast_mono,
            );
        } else {
            broadcast_mono[..actually_read.min(output_frames)]
                .copy_from_slice(&raw_broadcast_buffer[..actually_read.min(output_frames)]);
        }

        // 将单声道广播数据转换为输出声道数
        let mut broadcast_buffer = vec![0.0f32; data.len()];
        for frame in 0..output_frames {
            let mono_sample = broadcast_mono[frame];
            for ch in 0..output_channels {
                broadcast_buffer[frame * output_channels + ch] = mono_sample;
            }
        }

        // 从音乐引擎读取样本（已应用引擎音量和闪避）
        let mut music_buffer = vec![0.0f32; data.len()];
        music_engine.read_samples(&mut music_buffer);

        // 混音并转换为 i16（添加削波保护防止溢出）
        for i in 0..data.len() {
            let broadcast = broadcast_buffer[i];
            let music = music_buffer[i];
            let mixed = broadcast + music;
            // 先应用主音量，然后限制在 [-1.0, 1.0] 范围内，防止 i16 溢出
            let clamped = (mixed * master_vol).clamp(-1.0, 1.0);
            let sample = (clamped * i16::MAX as f32) as i16;
            data[i] = sample;
        }
    }

    /// U16 格式混音
    #[allow(clippy::needless_range_loop)]
    fn mix_audio_u16(
        data: &mut [u16],
        mixer_control: &Arc<Mutex<MixerControl>>,
        _broadcast_consumer: &Arc<BroadcastConsumer>,
        music_engine: &Arc<MusicEngine>,
        ducker: &Arc<Mutex<AudioDucker>>,
        output_sample_rate: u32,
        output_channels: u16,
    ) {
        // 每帧更新闪避器状态
        let mut d = ducker.lock();
        d.update();
        drop(d);

        // 从共享控制状态获取主音量
        let master_vol = mixer_control.lock().master_volume;

        // 从全局静态获取最新的消费者
        let current_consumer = crate::audio::broadcast_consumer();

        // 动态获取当前广播采样率
        let broadcast_sample_rate = crate::audio::broadcast_sample_rate();

        // 计算输出声道数和帧数
        let output_channels = output_channels as usize;
        let output_frames = data.len() / output_channels;

        // 广播数据是单声道，需要读取的样本数（基于帧数）
        let broadcast_frames_needed = if broadcast_sample_rate == output_sample_rate {
            output_frames
        } else {
            ((output_frames as f64) * (broadcast_sample_rate as f64 / output_sample_rate as f64))
                .ceil() as usize
                + 100
        };

        // 从无锁环形缓冲区读取广播音频（单声道）
        let mut raw_broadcast_buffer = vec![0.0f32; broadcast_frames_needed];
        let actually_read = read_from_ringbuf(&current_consumer, &mut raw_broadcast_buffer);

        // 重采样到目标采样率（仍然是单声道）
        let mut broadcast_mono = vec![0.0f32; output_frames];
        if broadcast_sample_rate != output_sample_rate {
            resample_linear(
                &raw_broadcast_buffer,
                broadcast_sample_rate,
                output_sample_rate,
                &mut broadcast_mono,
            );
        } else {
            broadcast_mono[..actually_read.min(output_frames)]
                .copy_from_slice(&raw_broadcast_buffer[..actually_read.min(output_frames)]);
        }

        // 将单声道广播数据转换为输出声道数
        let mut broadcast_buffer = vec![0.0f32; data.len()];
        for frame in 0..output_frames {
            let mono_sample = broadcast_mono[frame];
            for ch in 0..output_channels {
                broadcast_buffer[frame * output_channels + ch] = mono_sample;
            }
        }

        // 从音乐引擎读取样本（已应用引擎音量和闪避）
        let mut music_buffer = vec![0.0f32; data.len()];
        music_engine.read_samples(&mut music_buffer);

        // 混音并转换为 u16
        for i in 0..data.len() {
            let broadcast = broadcast_buffer[i];
            let music = music_buffer[i];
            let mixed = broadcast + music;
            // u16 范围: 0 到 65535，中心点 32768
            let sample = ((mixed * master_vol + 1.0) * 32768.0) as u16;
            data[i] = sample;
        }
    }

    /// 获取流配置
    #[allow(dead_code)]
    pub fn stream_config(&self) -> &StreamConfig {
        &self.config
    }

    /// 获取音乐引擎引用
    #[allow(dead_code)]
    pub fn music_engine(&self) -> &Arc<MusicEngine> {
        &self.music_engine
    }

    /// 获取环形缓冲区可用样本数
    #[allow(dead_code)]
    pub fn broadcast_available_samples(&self) -> usize {
        crate::audio::engine::BroadcastConsumer::available_samples_arc(&self.broadcast_consumer)
    }
}
