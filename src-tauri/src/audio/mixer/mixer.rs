//! 音频混音器
//!
//! 负责将广播音频和背景音乐混合后输出到扬声器。
//! 支持：
//! - 多音源混音（广播 + 背景音乐）
//! - 音频闪避（广播时自动降低背景音乐音量）
//! - 独立音量控制

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::ducker::AudioDucker;
use crate::audio::{BroadcastEngine, MusicEngine, MixerControl};
use crate::error::{AudioError, AudioResult};

/// 共享混音状态（用于监控）
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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
/// 使用 MusicEngine 的 read_samples 接口读取背景音乐样本。
pub struct AudioMixer {
    /// 音频输出流
    stream: Option<Stream>,
    /// 音频输出流配置
    config: StreamConfig,
    /// 混音器控制状态（与 HTTP API 共享）
    mixer_control: Arc<Mutex<MixerControl>>,
    /// 音频闪避器
    ducker: Arc<Mutex<AudioDucker>>,
    /// 广播引擎引用（使用 RwLock 支持动态重配置）
    broadcast_engine: Arc<RwLock<BroadcastEngine>>,
    /// 音乐引擎引用
    music_engine: Arc<MusicEngine>,
}

impl AudioMixer {
    /// 创建新的音频混合器
    pub fn new(
        broadcast_engine: Arc<RwLock<BroadcastEngine>>,
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

        tracing::info!(
            "音频输出设备: {}, 采样率: {}, 声道: {}",
            device.name().unwrap_or_default(),
            config.sample_rate.0,
            config.channels
        );

        let ducker = Arc::new(Mutex::new(AudioDucker::new(
            duck_volume,
            fade_in_ms,
            fade_out_ms,
        )));

        let err_fn = |err| tracing::error!("音频流错误: {}", err);

        // 根据采样格式创建流
        let stream = match sample_format {
            SampleFormat::F32 => {
                let mixer_control_clone = mixer_control.clone();
                let broadcast_clone = broadcast_engine.clone();
                let music_clone = music_engine.clone();
                let ducker_clone = ducker.clone();
                device.build_output_stream::<f32, _, _>(
                    &config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::mix_audio_f32(data, &mixer_control_clone, &broadcast_clone, &music_clone, &ducker_clone);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let mixer_control_clone = mixer_control.clone();
                let broadcast_clone = broadcast_engine.clone();
                let music_clone = music_engine.clone();
                let ducker_clone = ducker.clone();
                device.build_output_stream::<i16, _, _>(
                    &config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::mix_audio_i16(data, &mixer_control_clone, &broadcast_clone, &music_clone, &ducker_clone);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let mixer_control_clone = mixer_control.clone();
                let broadcast_clone = broadcast_engine.clone();
                let music_clone = music_engine.clone();
                let ducker_clone = ducker.clone();
                device.build_output_stream::<u16, _, _>(
                    &config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::mix_audio_u16(data, &mixer_control_clone, &broadcast_clone, &music_clone, &ducker_clone);
                    },
                    err_fn,
                    None,
                )
            }
            format => {
                return Err(AudioError::DeviceInit(format!("不支持的采样格式: {:?}", format)));
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
            broadcast_engine,
            music_engine,
        })
    }

    /// F32 格式混音
    fn mix_audio_f32(
        data: &mut [f32],
        mixer_control: &Arc<Mutex<MixerControl>>,
        broadcast_engine: &Arc<RwLock<BroadcastEngine>>,
        music_engine: &Arc<MusicEngine>,
        ducker: &Arc<Mutex<AudioDucker>>,
    ) {
        // 每帧更新闪避器状态
        let mut d = ducker.lock();
        d.update();
        drop(d);

        // 从共享控制状态获取主音量
        let master_vol = mixer_control.lock().master_volume;

        // 从广播引擎读取样本（已应用引擎音量）
        let mut broadcast_buffer = vec![0.0f32; data.len()];
        broadcast_engine.read().read_samples(&mut broadcast_buffer);

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
    fn mix_audio_i16(
        data: &mut [i16],
        mixer_control: &Arc<Mutex<MixerControl>>,
        broadcast_engine: &Arc<RwLock<BroadcastEngine>>,
        music_engine: &Arc<MusicEngine>,
        ducker: &Arc<Mutex<AudioDucker>>,
    ) {
        // 每帧更新闪避器状态
        let mut d = ducker.lock();
        d.update();
        drop(d);

        // 从共享控制状态获取主音量
        let master_vol = mixer_control.lock().master_volume;

        // 从广播引擎读取样本（已应用引擎音量）
        let mut broadcast_buffer = vec![0.0f32; data.len()];
        broadcast_engine.read().read_samples(&mut broadcast_buffer);

        // 从音乐引擎读取样本（已应用引擎音量和闪避）
        let mut music_buffer = vec![0.0f32; data.len()];
        music_engine.read_samples(&mut music_buffer);

        // 混音并转换为 i16
        for i in 0..data.len() {
            let broadcast = broadcast_buffer[i];
            let music = music_buffer[i];
            let mixed = broadcast + music;
            let sample = (mixed * master_vol * i16::MAX as f32) as i16;
            data[i] = sample;
        }
    }

    /// U16 格式混音
    fn mix_audio_u16(
        data: &mut [u16],
        mixer_control: &Arc<Mutex<MixerControl>>,
        broadcast_engine: &Arc<RwLock<BroadcastEngine>>,
        music_engine: &Arc<MusicEngine>,
        ducker: &Arc<Mutex<AudioDucker>>,
    ) {
        // 每帧更新闪避器状态
        let mut d = ducker.lock();
        d.update();
        drop(d);

        // 从共享控制状态获取主音量
        let master_vol = mixer_control.lock().master_volume;

        // 从广播引擎读取样本（已应用引擎音量）
        let mut broadcast_buffer = vec![0.0f32; data.len()];
        broadcast_engine.read().read_samples(&mut broadcast_buffer);

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
    pub fn stream_config(&self) -> &StreamConfig {
        &self.config
    }

    /// 获取广播引擎引用
    pub fn broadcast_engine(&self) -> &Arc<RwLock<BroadcastEngine>> {
        &self.broadcast_engine
    }

    /// 获取音乐引擎引用
    pub fn music_engine(&self) -> &Arc<MusicEngine> {
        &self.music_engine
    }
}
