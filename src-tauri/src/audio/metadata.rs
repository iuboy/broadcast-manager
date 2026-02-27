//! 音频元数据读取
//!
//! 使用 symphonia 库读取音频时长，使用 lofty 库读取 ID3/元数据标签。

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;
use rodio::Decoder;
use rodio::Source;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;

/// 获取音频文件时长（秒）
///
/// 首先尝试从 symphonia 元数据获取，如果失败则解码整个文件计算。
///
/// # 参数
/// - `path`: 音频文件路径
///
/// # 返回
/// 音频时长（秒），如果无法读取则返回 None
pub fn get_audio_duration(path: &Path) -> Option<f64> {
    // 方法1: 尝试从 symphonia 元数据获取
    if let Some(duration) = get_audio_duration_from_metadata(path) {
        return Some(duration);
    }

    // 方法2: 通过解码整个文件计算时长（较慢但可靠）
    tracing::info!("元数据无法获取时长，解码文件计算: {}", path.display());
    get_audio_duration_by_decoding(path)
}

/// 从元数据获取音频时长
fn get_audio_duration_from_metadata(path: &Path) -> Option<f64> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ).ok()?;

    let format = probed.format;

    for track in format.tracks() {
        if let Some(sample_rate) = track.codec_params.sample_rate {
            if let Some(n_frames) = track.codec_params.n_frames {
                let duration = n_frames as f64 / sample_rate as f64;
                return Some(duration);
            }
        }
    }

    None
}

/// 通过解码整个文件获取音频时长
fn get_audio_duration_by_decoding(path: &Path) -> Option<f64> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    // 创建解码器
    let decoder = Decoder::new(reader).ok()?;
    let sample_rate = decoder.sample_rate() as f64;
    let channels = decoder.channels() as f64;

    // 统计总样本数
    let mut total_samples = 0u64;
    let mut decoder = decoder;

    // 遍历所有样本
    loop {
        // 每次读取一批样本（避免一次读取太多）
        let mut count = 0u64;
        for _ in 0..10000 {
            if decoder.next().is_none() {
                break;
            }
            count += 1;
        }
        total_samples += count;
        if count < 10000 {
            break;
        }
    }

    // 计算时长 = 总样本数 / (采样率 * 声道数)
    let duration = total_samples as f64 / (sample_rate * channels);
    tracing::info!("解码完成，时长: {:.2} 秒 ({} 样本)", duration, total_samples);
    Some(duration)
}

/// 获取音频文件时长（Duration）
pub fn get_audio_duration_as_duration(path: &Path) -> Option<Duration> {
    let seconds = get_audio_duration(path)?;
    let secs = seconds as u64;
    let nanos = (seconds.fract() * 1e9) as u32;
    Some(Duration::new(secs, nanos))
}

/// 音频元数据（包含标题、艺术家、专辑、时长）
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<f64>,
}

/// 读取音频文件的完整元数据
///
/// 使用 lofty 库读取标签信息（标题、艺术家、专辑）
/// 使用 symphonia 库读取时长
pub fn read_audio_metadata(path: &Path) -> AudioMetadata {
    tracing::debug!("读取音频元数据: {}", path.display());

    let duration_secs = get_audio_duration(path);
    tracing::info!("音频时长: {:?} - {}", duration_secs, path.display());

    let mut metadata = AudioMetadata {
        title: None,
        artist: None,
        album: None,
        duration_secs,
    };

    // 从文件名获取标题（如果没有元数据）
    if metadata.title.is_none() {
        if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
            metadata.title = Some(file_stem.to_string());
        }
    }

    // 尝试使用 lofty 读取更详细的元数据标签
    // 使用 TaggedFileExt trait 来访问 tag
    use lofty::file::TaggedFileExt;
    use lofty::tag::Accessor;

    if let Ok(tagged_file) = lofty::read_from_path(path) {
        // 获取主要标签（primary_tag）或第一个标签（first_tag）
        if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
            // 使用 Accessor trait 的方法来读取标签信息
            if let Some(title) = tag.title() {
                if !title.is_empty() {
                    metadata.title = Some(title.to_string());
                }
            }

            if let Some(artist) = tag.artist() {
                if !artist.is_empty() {
                    metadata.artist = Some(artist.to_string());
                }
            }

            if let Some(album) = tag.album() {
                if !album.is_empty() {
                    metadata.album = Some(album.to_string());
                }
            }
        }
    }

    tracing::debug!("元数据读取完成: title={:?}, artist={:?}, album={:?}, duration={:?}",
        metadata.title, metadata.artist, metadata.album, metadata.duration_secs);

    metadata
}
