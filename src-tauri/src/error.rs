use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("无法初始化音频设备: {0}")]
    DeviceInit(String),

    #[error("无法播放音频: {0}")]
    Playback(String),

    #[error("解码错误: {0}")]
    Decode(String),

    #[error("文件不存在: {0}")]
    FileNotFound(String),

    #[error("初始化错误: {0}")]
    Init(String),
}

pub type AudioResult<T> = std::result::Result<T, AudioError>;

// 第三方库错误转换
impl From<cpal::BuildStreamError> for AudioError {
    fn from(e: cpal::BuildStreamError) -> Self {
        AudioError::DeviceInit(format!("无法创建音频流: {}", e))
    }
}

impl From<cpal::PlayStreamError> for AudioError {
    fn from(e: cpal::PlayStreamError) -> Self {
        AudioError::Playback(format!("无法播放音频流: {}", e))
    }
}

impl From<cpal::SupportedStreamConfigsError> for AudioError {
    fn from(e: cpal::SupportedStreamConfigsError) -> Self {
        AudioError::DeviceInit(format!("无法获取设备配置: {}", e))
    }
}
