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

// 从 actor 模块重导出（推荐使用）
pub use actor::{AudioActor, AudioMessage, AudioState, PlaybackProgress, PlaylistItemDto};

// 从 actor 模块导出 MixerControl（供 mixer 模块使用）
pub use actor::MixerControl;

// 从 engine 模块重导出
pub use engine::{BroadcastEngine, BroadcastStatus, DuckingConfig, MusicEngine, MusicState, MusicStatus};

// 从 mixer 模块重导出
pub use mixer::{AudioMixer, MixerState};

// 从 decoder 模块重导出
pub use decoder::OpusDecoderEngine;
