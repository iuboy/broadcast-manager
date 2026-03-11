//! 音频模块
//!
//! 提供：
//! - `actor`: Actor 模式封装（推荐使用）
//! - `engine`: 音频引擎（广播引擎、音乐引擎）
//! - `mixer`: 音频混音器
//! - `decoder`: 音频解码器
//! - `playlist`: 播放列表管理

pub mod actor;
pub mod decoder;
pub mod engine;
pub mod metadata;
pub mod mixer;
pub mod playlist;

// 从 actor 模块重导出
pub use actor::{
    AudioActor, AudioMessage, AudioState, MixerControl, PlaybackProgress, PlaylistItemDto,
};

// 从 actor 模块导出广播消费者
pub use actor::{broadcast_consumer, broadcast_sample_rate};
