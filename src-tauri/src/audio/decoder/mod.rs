//! 音频解码器模块
//!
//! 提供：
//! - `OpusDecoderEngine`: Opus 解码器
//! - `OpusConfig`: Opus 配置

pub mod opus;

pub use opus::{OpusConfig, OpusDecoderEngine};
