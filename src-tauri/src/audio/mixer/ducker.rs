use std::time::Instant;

/// 音频闪避状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DuckState {
    /// 正常播放
    Normal,
    /// 正在淡出（闪避）
    FadingOut,
    /// 已闪避
    Ducked,
    /// 正在淡入（恢复）
    FadingIn,
}

use serde::{Deserialize, Serialize};

/// 音频闪避器
pub struct AudioDucker {
    /// 当前状态
    state: DuckState,
    /// 当前音量系数
    current_volume: f32,
    /// 目标闪避音量
    duck_volume: f32,
    /// 正常音量
    normal_volume: f32,
    /// 淡出时间（毫秒）
    fade_out_ms: u64,
    /// 淡入时间（毫秒）
    fade_in_ms: u64,
    /// 淡入/淡出开始时间
    fade_start: Option<Instant>,
    /// 淡入/淡出起始音量
    fade_from_volume: f32,
}

impl AudioDucker {
    pub fn new(duck_volume: f32, fade_in_ms: u64, fade_out_ms: u64) -> Self {
        Self {
            state: DuckState::Normal,
            current_volume: 1.0,
            duck_volume,
            normal_volume: 1.0,
            fade_out_ms,
            fade_in_ms,
            fade_start: None,
            fade_from_volume: 1.0,
        }
    }

    /// 触发闪避（开始广播时调用）
    pub fn trigger_duck(&mut self) {
        if self.state == DuckState::Normal || self.state == DuckState::FadingIn {
            self.state = DuckState::FadingOut;
            self.fade_start = Some(Instant::now());
            self.fade_from_volume = self.current_volume;
            tracing::debug!("音频闪避开始，淡出到 {}", self.duck_volume);
        }
    }

    /// 恢复正常（结束广播时调用）
    pub fn trigger_normal(&mut self) {
        if self.state == DuckState::Ducked || self.state == DuckState::FadingOut {
            self.state = DuckState::FadingIn;
            self.fade_start = Some(Instant::now());
            self.fade_from_volume = self.current_volume;
            tracing::debug!("音频闪避结束，淡入到 {}", self.normal_volume);
        }
    }

    /// 更新音量（每帧调用）
    pub fn update(&mut self) {
        match self.state {
            DuckState::FadingOut => {
                if let Some(start) = self.fade_start {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let duration = self.fade_out_ms;

                    if elapsed >= duration {
                        self.current_volume = self.duck_volume;
                        self.state = DuckState::Ducked;
                        self.fade_start = None;
                        tracing::debug!("音频闪避完成");
                    } else {
                        let progress = elapsed as f32 / duration as f32;
                        // 使用对数插值实现平滑过渡
                        self.current_volume = self.interpolate_log(
                            self.fade_from_volume,
                            self.duck_volume,
                            progress,
                        );
                    }
                }
            }
            DuckState::FadingIn => {
                if let Some(start) = self.fade_start {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let duration = self.fade_in_ms;

                    if elapsed >= duration {
                        self.current_volume = self.normal_volume;
                        self.state = DuckState::Normal;
                        self.fade_start = None;
                        tracing::debug!("音频恢复正常");
                    } else {
                        let progress = elapsed as f32 / duration as f32;
                        self.current_volume = self.interpolate_log(
                            self.fade_from_volume,
                            self.normal_volume,
                            progress,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    /// 获取当前音量系数
    pub fn volume(&self) -> f32 {
        self.current_volume
    }

    /// 获取当前状态
    #[allow(dead_code)]
    pub fn state(&self) -> DuckState {
        self.state
    }

    /// 对数插值（dB 空间插值 + smoothstep）
    ///
    /// 人耳对音量的感知是对数的，所以在 dB 空间进行线性插值
    /// 可以实现更自然的听感。smoothstep 曲线使过渡更加平滑。
    fn interpolate_log(&self, from: f32, to: f32, t: f32) -> f32 {
        const MIN_VOL: f32 = 0.0001; // -80dB 下限

        let from_safe = from.max(MIN_VOL);
        let to_safe = to.max(MIN_VOL);

        // smoothstep 曲线使过渡更自然
        let smooth_t = t * t * (3.0 - 2.0 * t);

        // 在 dB 空间进行线性插值
        let from_db = 20.0 * from_safe.log10();
        let to_db = 20.0 * to_safe.log10();
        let interp_db = from_db + (to_db - from_db) * smooth_t;

        10f32.powf(interp_db / 20.0)
    }

    /// 设置闪避音量
    pub fn set_duck_volume(&mut self, volume: f32) {
        self.duck_volume = volume.clamp(0.0, 1.0);
    }

    /// 设置淡入淡出时间
    pub fn set_fade_times(&mut self, fade_in_ms: u64, fade_out_ms: u64) {
        self.fade_in_ms = fade_in_ms;
        self.fade_out_ms = fade_out_ms;
    }

    /// 获取淡入时间
    pub fn fade_in_ms(&self) -> u64 {
        self.fade_in_ms
    }

    /// 获取淡出时间
    pub fn fade_out_ms(&self) -> u64 {
        self.fade_out_ms
    }
}

impl Default for AudioDucker {
    fn default() -> Self {
        Self::new(0.3, 400, 600)
    }
}
