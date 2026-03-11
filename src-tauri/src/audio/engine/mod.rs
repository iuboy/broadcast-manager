//! 音频引擎模块
//!
//! 提供：
//! - `BroadcastEngine`: 广播音频引擎，接收客户端音频数据
//! - `BroadcastEngineLockFree`: 无锁广播引擎包装器，使用 SPSC 环形缓冲区
//! - `MusicEngine`: 背景音乐引擎，管理播放列表和音乐播放
//! - `MusicState`: 音乐播放状态
//! - `DuckingConfig`: 音频闪避配置

mod broadcast;
mod broadcast_lockfree;
mod music;

pub use broadcast_lockfree::{read_from_ringbuf, BroadcastConsumer, BroadcastEngineLockFree};
pub use music::{DuckingConfig, MusicEngine};
