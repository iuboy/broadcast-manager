// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! # 广播服务管理器 - Tauri 应用入口
//!
//! 这是一个内嵌广播服务的 Tauri 应用，无需额外的 Sidecar 依赖。
//!
//! ## 功能
//! - WebSocket 服务器 - 接收客户端音频数据
//! - HTTP API - 提供服务状态查询
//! - 音频播放 - 播放音乐并混音广播音频
//! - 音频闪避 - 广播时自动降低音乐音量
//! - 播放列表管理
//! - 开机自启动支持
//!
//! ## 技术栈
//! - Tauri 2.x - 跨平台桌面应用框架
//! - Axum - Web 框架（WebSocket + HTTP）
//! - Rodio - 音频播放
//! - Actor 模式 - 音频子系统管理

fn main() {
    broadcast_manager_lib::run()
}
