use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::metadata::read_audio_metadata;

/// 播放模式
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum PlayMode {
    /// 顺序播放
    Sequential,
    /// 循环播放
    #[default]
    Loop,
    /// 单曲循环
    SingleLoop,
    /// 随机播放
    Shuffle,
}

/// 播放列表项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<u64>,
    pub path: PathBuf,
}

impl PlaylistItem {
    pub fn from_path(path: PathBuf) -> Self {
        // 读取音频元数据
        let audio_metadata = read_audio_metadata(&path);

        Self {
            id: Uuid::new_v4().to_string(),
            title: audio_metadata.title.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string()
            }),
            artist: audio_metadata.artist,
            album: audio_metadata.album,
            duration: audio_metadata.duration_secs.map(|s| s as u64),
            path,
        }
    }
}

/// 播放列表
pub struct Playlist {
    items: RwLock<Vec<PlaylistItem>>,
    current_index: RwLock<usize>,
    play_mode: RwLock<PlayMode>,
    shuffle_order: RwLock<Vec<usize>>,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(Vec::new()),
            current_index: RwLock::new(0),
            play_mode: RwLock::new(PlayMode::Loop),
            shuffle_order: RwLock::new(Vec::new()),
        }
    }

    /// 添加项目
    pub fn add(&self, item: PlaylistItem) -> String {
        let id = item.id.clone();
        self.items.write().push(item);
        self.rebuild_shuffle_order();
        id
    }

    /// 移除项目
    pub fn remove(&self, id: &str) -> bool {
        let mut items = self.items.write();
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            items.remove(pos);
            // 调整当前索引
            let mut current = self.current_index.write();
            if *current >= pos && *current > 0 {
                *current -= 1;
            }
            self.rebuild_shuffle_order();
            true
        } else {
            false
        }
    }

    /// 清空列表
    pub fn clear(&self) {
        self.items.write().clear();
        *self.current_index.write() = 0;
        self.shuffle_order.write().clear();
    }

    /// 获取当前项
    pub fn current(&self) -> Option<PlaylistItem> {
        let items = self.items.read();
        let index = *self.current_index.read();
        items.get(index).cloned()
    }

    /// 获取下一首
    pub fn next(&self) -> Option<PlaylistItem> {
        let items = self.items.read();
        if items.is_empty() {
            return None;
        }

        let mode = *self.play_mode.read();
        let mut current = self.current_index.write();

        match mode {
            PlayMode::Sequential => {
                if *current + 1 < items.len() {
                    *current += 1;
                }
            }
            PlayMode::Loop => {
                *current = (*current + 1) % items.len();
            }
            PlayMode::SingleLoop => {
                // 保持当前位置
            }
            PlayMode::Shuffle => {
                let shuffle_order = self.shuffle_order.read();
                if !shuffle_order.is_empty() {
                    // 找到当前在 shuffle_order 中的位置
                    if let Some(shuffle_pos) = shuffle_order.iter().position(|&i| i == *current) {
                        let next_shuffle = (shuffle_pos + 1) % shuffle_order.len();
                        *current = shuffle_order[next_shuffle];
                    }
                }
            }
        }

        items.get(*current).cloned()
    }

    /// 获取上一首
    pub fn previous(&self) -> Option<PlaylistItem> {
        let items = self.items.read();
        if items.is_empty() {
            return None;
        }

        let mode = *self.play_mode.read();
        let mut current = self.current_index.write();

        match mode {
            PlayMode::Sequential => {
                if *current > 0 {
                    *current -= 1;
                }
            }
            PlayMode::Loop => {
                *current = if *current == 0 {
                    items.len() - 1
                } else {
                    *current - 1
                };
            }
            PlayMode::SingleLoop => {
                // 保持当前位置
            }
            PlayMode::Shuffle => {
                let shuffle_order = self.shuffle_order.read();
                if !shuffle_order.is_empty() {
                    if let Some(shuffle_pos) = shuffle_order.iter().position(|&i| i == *current) {
                        let prev_shuffle = if shuffle_pos == 0 {
                            shuffle_order.len() - 1
                        } else {
                            shuffle_pos - 1
                        };
                        *current = shuffle_order[prev_shuffle];
                    }
                }
            }
        }

        items.get(*current).cloned()
    }

    /// 设置播放位置
    pub fn set_current(&self, id: &str) -> bool {
        let items = self.items.read();
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            *self.current_index.write() = pos;
            true
        } else {
            false
        }
    }

    /// 设置播放模式
    pub fn set_play_mode(&self, mode: PlayMode) {
        *self.play_mode.write() = mode;
        if mode == PlayMode::Shuffle {
            self.rebuild_shuffle_order();
        }
    }

    /// 获取播放模式
    #[allow(dead_code)]
    pub fn play_mode(&self) -> PlayMode {
        *self.play_mode.read()
    }

    /// 获取所有项目
    pub fn items(&self) -> Vec<PlaylistItem> {
        self.items.read().clone()
    }

    /// 获取列表长度
    pub fn len(&self) -> usize {
        self.items.read().len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.items.read().is_empty()
    }

    /// 获取当前索引
    pub fn current_index(&self) -> usize {
        *self.current_index.read()
    }

    /// 重建随机顺序
    fn rebuild_shuffle_order(&self) {
        let items = self.items.read();
        let mut order: Vec<usize> = (0..items.len()).collect();

        // 简单的 Fisher-Yates 洗牌
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let mut rng_state = seed;
        for i in (1..order.len()).rev() {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng_state as usize) % (i + 1);
            order.swap(i, j);
        }

        *self.shuffle_order.write() = order;
    }

    /// 重排序
    #[allow(dead_code)]
    pub fn reorder(&self, from_index: usize, to_index: usize) -> bool {
        let mut items = self.items.write();
        if from_index >= items.len() || to_index >= items.len() {
            return false;
        }

        let item = items.remove(from_index);
        items.insert(to_index, item);

        // 更新当前索引
        let mut current = self.current_index.write();
        if *current == from_index {
            *current = to_index;
        } else if from_index < *current && to_index >= *current {
            *current += 1;
        } else if from_index > *current && to_index <= *current {
            *current -= 1;
        }

        true
    }
}

impl Default for Playlist {
    fn default() -> Self {
        Self::new()
    }
}
