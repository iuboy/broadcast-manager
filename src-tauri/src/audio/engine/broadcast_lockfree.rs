//! 无锁广播引擎包装器
//!
//! 使用 ringbuf SPSC 环形缓冲区实现跨线程无锁通信。
//!
//! 架构：
//! - Actor 线程（生产者）：解码音频数据 -> 写入环形缓冲区
//! - 音频回调线程（消费者）：从环形缓冲区读取 -> 输出到扬声器

use ringbuf::traits::*;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};

use super::broadcast::{AudioCodec, BroadcastEngine};
use crate::audio::decoder::opus::{OpusConfig, OpusDecoderEngine};
use crate::error::{AudioError, AudioResult};

/// 全局音频样本丢弃计数（用于监控缓冲区溢出）
static DROPPED_SAMPLES: AtomicU64 = AtomicU64::new(0);

/// 无锁广播引擎（线程安全包装器）
///
/// 内部使用 SPSC 环形缓冲区实现跨线程无锁通信。
/// - Actor 线程调用 `receive_pcm()` 写入数据
/// - 音频回调线程调用 `read_samples()` 读取数据
pub struct BroadcastEngineLockFree {
    /// 核心广播引擎（用于状态管理）
    engine: BroadcastEngine,
    /// 环形缓冲区生产者（Actor 线程使用）
    producer: ringbuf::HeapProd<f32>,
    /// Opus 解码器（如果使用 Opus 编解码）
    opus_decoder: Option<OpusDecoderEngine>,
    /// 采样率
    sample_rate: u32,
    /// 声道数
    channels: u16,
    /// 编解码格式
    codec: AudioCodec,
}

/// 环形缓冲区消费者（线程安全）
#[derive(Debug)]
pub struct BroadcastConsumer {
    pub consumer: UnsafeCell<ringbuf::HeapCons<f32>>,
}

// SAFETY: ringbuf::HeapCons 的所有操作都使用原子操作，可以安全地在线程间共享
unsafe impl Send for BroadcastConsumer {}
unsafe impl Sync for BroadcastConsumer {}

impl BroadcastConsumer {
    /// 获取消费者内部可变引用
    ///
    /// # Safety
    /// 调用者必须确保：
    /// 1. 没有其他线程同时访问此消费者的可变引用
    /// 2. 在 SPSC 模式下，只有一个生产者和一个消费者
    #[inline]
    #[allow(clippy::mut_from_ref)]
    fn get(&self) -> &mut ringbuf::HeapCons<f32> {
        // SAFETY: 在音频回调中，只有一个线程（音频回调线程）会调用此方法
        // 在 SPSC 环形缓冲区中，消费者只被一个线程访问
        unsafe { &mut *self.consumer.get() }
    }

    /// 获取可用样本数
    #[allow(dead_code)]
    pub fn available_samples_arc(this: &std::sync::Arc<Self>) -> usize {
        use ringbuf::traits::observer::Observer;
        unsafe { &*this.consumer.get() }.occupied_len()
    }

    /// 读取音频样本（音频回调线程调用）
    pub fn read_samples(&self, output: &mut [f32]) -> usize {
        use ringbuf::traits::consumer::Consumer;
        let consumer = self.get();
        let mut read_count = 0;
        for sample in output.iter_mut() {
            match consumer.try_pop() {
                Some(s) => {
                    *sample = s;
                    read_count += 1;
                }
                None => {
                    *sample = 0.0;
                }
            }
        }
        read_count
    }

    /// 获取可用样本数
    #[allow(dead_code)]
    pub fn available_samples(&self) -> usize {
        use ringbuf::traits::observer::Observer;
        self.get().occupied_len()
    }
}

impl BroadcastEngineLockFree {
    /// 创建新的无锁广播引擎
    ///
    /// # 参数
    /// - `sample_rate`: 采样率
    /// - `channels`: 声道数
    /// - `codec`: 编解码格式
    /// - `producer`: 环形缓冲区生产者（需从外部创建并分割）
    pub fn new_with_producer(
        sample_rate: u32,
        channels: u16,
        codec: String,
        producer: ringbuf::HeapProd<f32>,
    ) -> Self {
        let audio_codec = match codec.to_lowercase().as_str() {
            "opus" => AudioCodec::Opus,
            _ => AudioCodec::Pcm,
        };

        let opus_decoder = if audio_codec == AudioCodec::Opus {
            match OpusDecoderEngine::new(OpusConfig::new(sample_rate, channels)) {
                Ok(decoder) => {
                    tracing::info!("Opus 解码器初始化: {}Hz", sample_rate);
                    Some(decoder)
                }
                Err(e) => {
                    tracing::error!("Opus 解码器失败: {}, 使用 PCM", e);
                    None
                }
            }
        } else {
            None
        };

        let engine = BroadcastEngine::new(sample_rate, channels, codec);
        tracing::info!(
            "广播引擎创建: {}Hz {}ch {:?}",
            sample_rate,
            channels,
            audio_codec
        );

        Self {
            engine,
            producer,
            opus_decoder,
            sample_rate,
            channels,
            codec: audio_codec,
        }
    }

    /// 分离环形缓冲区并创建引擎
    ///
    /// 静态方法，用于创建环形缓冲区、分割它，并返回 (引擎, 消费者)。
    ///
    /// # 参数
    /// - `sample_rate`: 采样率
    /// - `channels`: 声道数
    /// - `codec`: 编解码格式
    /// - `buffer_capacity_ms`: 缓冲区容量（毫秒）
    ///
    /// # 返回
    /// (引擎, 消费者)，消费者可以传递给音频回调线程
    pub fn create(
        sample_rate: u32,
        channels: u16,
        codec: String,
        buffer_capacity_ms: usize,
    ) -> (Self, BroadcastConsumer) {
        // 计算缓冲区容量（样本数）
        let capacity = (sample_rate as usize) * (channels as usize) * buffer_capacity_ms / 1000;

        let ringbuf = ringbuf::HeapRb::<f32>::new(capacity);
        let (producer, consumer) = ringbuf.split();

        let engine = Self::new_with_producer(sample_rate, channels, codec, producer);
        let broadcast_consumer = BroadcastConsumer {
            consumer: UnsafeCell::new(consumer),
        };

        (engine, broadcast_consumer)
    }

    /// 接收音频数据（Actor 线程调用）
    pub fn receive_pcm(&mut self, data: &[u8]) -> AudioResult<()> {
        use ringbuf::traits::producer::Producer;
        let volume = self.engine.volume();

        match self.codec {
            AudioCodec::Pcm => {
                let mut dropped_in_call = 0;
                for chunk in data.chunks(2) {
                    if chunk.len() == 2 {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                        let normalized = (sample as f32 / i16::MAX as f32) * volume;
                        if self.producer.try_push(normalized).is_err() {
                            dropped_in_call += 1;
                        }
                    }
                }
                if dropped_in_call > 0 {
                    let total =
                        DROPPED_SAMPLES.fetch_add(dropped_in_call as u64, Ordering::Relaxed);
                    // 每丢弃约 10000 个样本记录一次警告（约 100ms @ 48kHz）
                    if total % 10000 < dropped_in_call as u64 {
                        tracing::warn!(
                            "音频缓冲区溢出：已丢弃 {} 个样本（约 {:.2} 秒音频）",
                            total,
                            total as f64 / 48000.0
                        );
                    }
                }
                Ok(())
            }
            AudioCodec::Opus => {
                if let Some(ref mut decoder) = self.opus_decoder {
                    match decoder.decode(data) {
                        Ok(samples) => {
                            let mut dropped_in_call = 0;
                            for sample in samples {
                                let scaled = sample * volume;
                                if self.producer.try_push(scaled).is_err() {
                                    dropped_in_call += 1;
                                }
                            }
                            if dropped_in_call > 0 {
                                let total = DROPPED_SAMPLES
                                    .fetch_add(dropped_in_call as u64, Ordering::Relaxed);
                                if total % 10000 < dropped_in_call as u64 {
                                    tracing::warn!(
                                        "音频缓冲区溢出：已丢弃 {} 个样本（约 {:.2} 秒音频）",
                                        total,
                                        total as f32 / 48000.0
                                    );
                                }
                            }
                            Ok(())
                        }
                        Err(e) => {
                            tracing::warn!("Opus 解码失败: {}", e);
                            Err(e)
                        }
                    }
                } else {
                    Err(AudioError::Decode("Opus 解码器未初始化".to_string()))
                }
            }
        }
    }

    /// 重新配置
    pub fn reconfigure(
        &mut self,
        codec: &str,
        sample_rate: u32,
        channels: u16,
        buffer_capacity_ms: usize,
    ) -> BroadcastConsumer {
        let new_codec = match codec.to_lowercase().as_str() {
            "opus" => AudioCodec::Opus,
            _ => AudioCodec::Pcm,
        };

        self.sample_rate = sample_rate;
        self.channels = channels;
        self.codec = new_codec;

        if new_codec == AudioCodec::Opus {
            match OpusDecoderEngine::new(OpusConfig::new(sample_rate, channels)) {
                Ok(decoder) => {
                    tracing::info!("Opus 解码器重新配置: {}Hz", sample_rate);
                    self.opus_decoder = Some(decoder);
                }
                Err(e) => {
                    tracing::error!("Opus 解码器失败: {}, 回退到 PCM", e);
                    self.codec = AudioCodec::Pcm;
                    self.opus_decoder = None;
                }
            }
        } else {
            self.opus_decoder = None;
        }

        self.engine.reconfigure(codec, sample_rate, channels);

        let capacity = (sample_rate as usize) * (channels as usize) * buffer_capacity_ms / 1000;
        let ringbuf = ringbuf::HeapRb::<f32>::new(capacity);
        let (producer, consumer) = ringbuf.split();
        self.producer = producer;

        tracing::info!("广播引擎重新配置: {} {}Hz", codec, sample_rate);

        BroadcastConsumer {
            consumer: UnsafeCell::new(consumer),
        }
    }

    /// 设置音量
    pub fn set_volume(&mut self, volume: f32) {
        self.engine.set_volume(volume);
    }

    /// 获取音量
    #[allow(dead_code)]
    pub fn volume(&self) -> f32 {
        self.engine.volume()
    }

    /// 开始播放
    #[allow(dead_code)]
    pub fn start(&mut self) {
        self.engine.start();
    }

    /// 停止播放
    #[allow(dead_code)]
    pub fn stop(&mut self) {
        self.engine.stop();
    }

    /// 是否正在播放
    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        self.engine.is_playing()
    }

    /// 清空缓冲区
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.engine.clear();
        if let Some(ref mut decoder) = self.opus_decoder {
            let _ = decoder.reset();
        }
    }

    /// 获取缓冲区大小（样本数）
    ///
    /// 返回生产者队列中的估计样本数。
    #[allow(dead_code)]
    pub fn buffer_size(&self) -> usize {
        use ringbuf::traits::observer::Observer;
        self.producer.occupied_len()
    }

    /// 获取编解码格式
    #[allow(dead_code)]
    pub fn codec(&self) -> AudioCodec {
        self.codec
    }

    /// 获取采样率
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 获取声道数
    #[allow(dead_code)]
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// 获取当前编解码格式名称
    #[allow(dead_code)]
    pub fn codec_name(&self) -> &'static str {
        match self.codec {
            AudioCodec::Pcm => "PCM",
            AudioCodec::Opus => {
                if self.opus_decoder.is_some() {
                    "Opus"
                } else {
                    "Opus (不可用)"
                }
            }
        }
    }

    /// 检查 Opus 解码器是否可用
    #[allow(dead_code)]
    pub fn is_opus_available(&self) -> bool {
        self.codec == AudioCodec::Opus && self.opus_decoder.is_some()
    }

    /// 获取广播状态（用于监控）
    #[allow(dead_code)]
    pub fn get_status(&self) -> super::broadcast::BroadcastStatus {
        let mut status = self.engine.get_status();
        status.buffer_size = self.buffer_size();
        status
    }
}

/// 辅助函数：从消费者读取样本（用于兼容旧接口）
///
/// 用于音频混音器中读取广播音频。
pub fn read_from_ringbuf(consumer: &BroadcastConsumer, output: &mut [f32]) -> usize {
    consumer.read_samples(output)
}
