//! 音频引擎模块
//!
//! 提供：
//! - `BroadcastEngine`: 广播音频引擎，接收客户端音频数据
//! - `MusicEngine`: 背景音乐引擎，管理播放列表和音乐播放
//! - `MusicState`: 音乐播放状态
//! - `DuckingConfig`: 音频闪避配置

mod broadcast;
mod music;

pub use broadcast::{BroadcastEngine, BroadcastStatus};
pub use music::{DuckingConfig, MusicEngine, MusicState, MusicStatus};
