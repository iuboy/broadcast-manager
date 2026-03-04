//! 音频混音器模块
//!
//! 提供：
//! - `AudioMixer`: 主混音器，负责混合广播音频和背景音乐
//! - `AudioDucker`: 音频闪避器，实现平滑的音量淡入淡出
//! - `DuckState`: 闪避状态枚举
//! - `MixerState`: 混音器状态（用于监控）

mod ducker;
mod mixer;

pub use ducker::{AudioDucker, DuckState};
pub use mixer::AudioMixer;
