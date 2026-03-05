//! # 广播服务管理器 - Tauri 应用库
//!
//! 本模块提供 Tauri 应用的核心功能，包括内嵌广播服务和音频管理。
//!
//! ## 主要模块
//! - `audio` - 音频子系统（Actor 模式）
//! - `config` - 配置管理
//! - `error` - 错误类型定义
//! - `server` - WebSocket 和 HTTP 服务器
//!
//! ## Tauri 命令分类
//!
//! ### 音频控制
//! - `get_audio_state` - 获取音频状态
//! - `play_music` / `pause_music` / `stop_music` - 播放控制
//! - `next_track` / `previous_track` - 切换曲目
//! - `set_music_volume` / `set_broadcast_volume` / `set_master_volume` - 音量控制
//!
//! ### 闪避功能
//! - `start_ducking` / `stop_ducking` - 启用/禁用闪避
//! - `set_ducking_enabled` - 设置闪避开关
//! - `set_ducking_volume` - 设置闪避音量
//! - `set_ducking_fade_in_ms` / `set_ducking_fade_out_ms` - 设置淡入淡出时间
//!
//! ### 播放列表
//! - `add_track` / `remove_track` / `clear_playlist` - 播放列表管理
//! - `get_playlist` - 获取播放列表
//! - `scan_directory` - 扫描目录添加音乐
//!
//! ### 服务器配置
//! - `get_service_status` - 获取服务状态
//! - `get_server_config` / `update_server_config` - 服务器配置
//! - `reset_all_config` - 重置配置
//!
//! ### 自启动
//! - `enable_autostart` / `disable_autostart` - 开机自启动控制
//! - `is_autostart_enabled` - 检查自启动状态

mod audio;
mod config;
mod error;
mod server;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};

use audio::{AudioActor, AudioMessage, AudioState, PlaybackProgress, PlaylistItemDto};
use config::Config;
use server::broadcast::BroadcastManager;
use tracing_subscriber::prelude::*;

/// 全局音频 Actor 实例
static AUDIO_ACTOR: OnceLock<AudioActor> = OnceLock::new();

/// 全局配置（可变，用于运行时更新和保存）
static CONFIG: OnceLock<Arc<Mutex<Config>>> = OnceLock::new();

/// 获取音频 Actor 全局实例
pub fn audio() -> &'static AudioActor {
    AUDIO_ACTOR.get().expect("Audio Actor not initialized")
}

/// 尝试获取音频 Actor，返回 Option（用于可能未初始化的场景）
pub fn try_audio() -> Option<&'static AudioActor> {
    AUDIO_ACTOR.get()
}

/// 获取全局配置
pub fn get_config() -> &'static Arc<Mutex<Config>> {
    CONFIG.get().expect("Config not initialized")
}

/// 尝试获取全局配置，返回 Option（用于可能未初始化的场景）
pub fn try_get_config() -> Option<&'static Arc<Mutex<Config>>> {
    CONFIG.get()
}

/// 保存运行时配置
fn save_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = try_get_config()
        .ok_or("配置未初始化")?
        .lock()
        .map_err(|e| format!("获取配置锁失败: {}", e))?;
    let config_clone = (*config).clone();
    drop(config);
    config_clone.save()
}

/// 更新并保存配置的辅助函数
fn update_config<F>(updater: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&mut Config),
{
    let mut config = try_get_config()
        .ok_or("配置未初始化")?
        .lock()
        .map_err(|e| format!("获取配置锁失败: {}", e))?;
    updater(&mut config);
    drop(config);
    save_config()
}

/// 保存播放列表到配置
fn save_playlist_to_config() {
    let items = audio().get_playlist_items();
    let paths: Vec<String> = items.iter().filter_map(|item| item.path.clone()).collect();
    if let Err(e) = update_config(|c| c.playlist = paths) {
        tracing::error!("保存播放列表到配置失败: {}", e);
    }
}

/// 检查客户端 IP 是否在白名单中
///
/// 环回地址（127.0.0.1, ::1, localhost）总是被允许
/// 如果白名单未启用，也允许所有地址
fn is_client_allowed(client_ip: &str) -> bool {
    let config = get_config().lock().unwrap();
    let whitelist = &config.server.client_whitelist;

    // 如果未启用白名单，允许所有地址
    if !whitelist.enabled {
        return true;
    }

    // 硬编码允许的环回地址
    let loopback_addresses = ["127.0.0.1", "::1", "localhost", "0.0.0.0"];

    // 首先检查是否为环回地址
    for loopback in &loopback_addresses {
        if client_ip == *loopback {
            return true;
        }
    }

    // 检查是否在白名单中
    for allowed in &whitelist.allowed_addresses {
        if client_ip == allowed {
            return true;
        }

        // 支持简单的 IP 段匹配（例如 192.168.1.）
        if allowed.ends_with('.') && client_ip.starts_with(allowed) {
            return true;
        }

        // 支持通配符 0.0.0.0 匹配所有地址
        if allowed == "0.0.0.0" {
            return true;
        }
    }

    false
}

// ===== Tauri 命令 =====

/// 获取音频状态
#[tauri::command]
fn get_audio_state() -> AudioState {
    audio().get_state()
}

/// 播放音乐
#[tauri::command]
fn play_music() {
    audio().play();
}

/// 暂停音乐
#[tauri::command]
fn pause_music() {
    audio().pause();
}

/// 停止音乐
#[tauri::command]
fn stop_music() {
    audio().stop();
}

/// 下一首
#[tauri::command]
fn next_track() {
    audio().next();
}

/// 上一首
#[tauri::command]
fn previous_track() {
    audio().previous();
}

/// 播放指定索引的曲目
#[tauri::command]
fn play_track_at(index: usize) -> Result<(), String> {
    audio().play_track_at(index)
}

/// 设置音乐音量
#[tauri::command]
fn set_music_volume(volume: f32) {
    audio().set_music_volume(volume);
    let _ = update_config(|c| c.runtime.music_volume = volume);
}

/// 设置广播音量
#[tauri::command]
fn set_broadcast_volume(volume: f32) {
    audio().set_broadcast_volume(volume);
    let _ = update_config(|c| c.runtime.broadcast_volume = volume);
}

/// 设置主音量
#[tauri::command]
fn set_master_volume(volume: f32) {
    audio().set_master_volume(volume);
    let _ = update_config(|c| c.runtime.master_volume = volume);
}

/// 开始音频闪避
#[tauri::command]
fn start_ducking() {
    audio().start_ducking();
}

/// 停止音频闪避
#[tauri::command]
fn stop_ducking() {
    audio().stop_ducking();
}

/// 设置闪避功能是否启用
#[tauri::command]
fn set_ducking_enabled(enabled: bool) {
    audio().set_ducking_enabled(enabled);
    let _ = update_config(|c| c.runtime.ducking_enabled = enabled);
}

/// 添加曲目
#[tauri::command]
fn add_track(path: String) {
    use crossbeam_channel::unbounded;
    let (reply_tx, reply_rx) = unbounded();
    audio().send(AudioMessage::AddTrack { path, reply: reply_tx });
    // 等待消息处理完成，然后保存配置
    let _ = reply_rx.recv_timeout(std::time::Duration::from_secs(1));
    save_playlist_to_config();
}

/// 移除曲目
#[tauri::command]
fn remove_track(index: usize) {
    use crossbeam_channel::unbounded;
    let (reply_tx, reply_rx) = unbounded();
    audio().send(AudioMessage::RemoveTrack { index, reply: reply_tx });
    // 等待消息处理完成，然后保存配置
    let _ = reply_rx.recv_timeout(std::time::Duration::from_secs(1));
    save_playlist_to_config();
}

/// 清空播放列表
#[tauri::command]
fn clear_playlist() {
    use crossbeam_channel::unbounded;
    let (reply_tx, reply_rx) = unbounded();
    audio().send(AudioMessage::ClearPlaylist { reply: reply_tx });
    // 等待消息处理完成，然后保存配置
    let _ = reply_rx.recv_timeout(std::time::Duration::from_secs(1));
    save_playlist_to_config();
}

/// 获取播放列表
#[tauri::command]
fn get_playlist() -> Vec<PlaylistItemDto> {
    audio().get_playlist_items()
}

/// 获取播放进度
#[tauri::command]
fn get_playback_progress() -> PlaybackProgress {
    audio().get_playback_progress()
}

/// 设置播放模式
#[tauri::command]
fn set_play_mode(mode: String) -> Result<(), String> {
    audio().set_play_mode(&mode)
}

/// 扫描目录
#[tauri::command]
fn scan_directory(path: String) -> Result<usize, String> {
    let count = audio().scan_directory(&path)?;
    // scan_directory 已经等待扫描完成，直接保存配置
    save_playlist_to_config();
    Ok(count)
}

/// 设置闪避音量
#[tauri::command]
fn set_ducking_volume(volume: f32) {
    audio().set_ducking_volume(volume);
    let _ = update_config(|c| c.runtime.ducking_volume = volume);
}

/// 设置闪避淡入时间
#[tauri::command]
fn set_ducking_fade_in_ms(ms: u64) {
    audio().set_ducking_fade_in_ms(ms);
    let _ = update_config(|c| c.runtime.ducking_fade_in_ms = ms);
}

/// 设置闪避淡出时间
#[tauri::command]
fn set_ducking_fade_out_ms(ms: u64) {
    audio().set_ducking_fade_out_ms(ms);
    let _ = update_config(|c| c.runtime.ducking_fade_out_ms = ms);
}

/// 获取服务状态
#[tauri::command]
fn get_service_status() -> serde_json::Value {
    // 音频 Actor 总是运行状态
    let config = get_config().lock().unwrap();
    serde_json::json!({
        "status": "running",
        "state": "running",
        "port": config.server.port,
        "error": serde_json::Value::Null
    })
}

/// 获取服务器配置
#[tauri::command]
fn get_server_config() -> ServerConfigDto {
    let config = get_config().lock().unwrap();
    ServerConfigDto {
        bind_address: config.server.bind_address.clone(),
        port: config.server.port,
        max_connections: config.server.max_connections,
        whitelist_enabled: config.server.client_whitelist.enabled,
        whitelist_addresses: config.server.client_whitelist.allowed_addresses.clone(),
    }
}

/// 更新服务器配置
#[tauri::command]
fn update_server_config(config_dto: ServerConfigDto) -> Result<(), String> {
    // 验证端口范围
    if config_dto.port == 0 {
        return Err("端口号不能为 0".to_string());
    }

    // 验证最大连接数
    if config_dto.max_connections == 0 {
        return Err("最大连接数必须大于 0".to_string());
    }
    if config_dto.max_connections > 1000 {
        return Err("最大连接数不能超过 1000".to_string());
    }

    // 验证绑定地址格式
    if config_dto.bind_address.is_empty() {
        return Err("绑定地址不能为空".to_string());
    }
    // 尝试解析为 IP 地址
    if config_dto.bind_address != "0.0.0.0" &&
       config_dto.bind_address != "localhost" &&
       config_dto.bind_address.parse::<std::net::IpAddr>().is_err() {
        return Err(format!("无效的绑定地址: {}", config_dto.bind_address));
    }

    // 验证白名单地址格式
    for addr in &config_dto.whitelist_addresses {
        if addr.is_empty() {
            return Err("白名单地址不能为空".to_string());
        }
        // 尝试解析为 IP 地址或检查是否为 localhost
        if addr != "localhost" &&
           addr != "0.0.0.0" &&
           addr.parse::<std::net::IpAddr>().is_err() {
            return Err(format!("无效的白名单地址: {}", addr));
        }
    }

    update_config(|c| {
        c.server.bind_address = config_dto.bind_address;
        c.server.port = config_dto.port;
        c.server.max_connections = config_dto.max_connections;
        c.server.client_whitelist.enabled = config_dto.whitelist_enabled;
        c.server.client_whitelist.allowed_addresses = config_dto.whitelist_addresses;
    }).map_err(|e| e.to_string())
}

/// 重置所有配置为默认值
#[tauri::command]
fn reset_all_config() -> Result<(), String> {
    let default_config = Config::default();
    let mut config = get_config().lock().unwrap();
    *config = default_config;
    drop(config);

    // 保存默认配置
    save_config().map_err(|e| e.to_string())?;

    // 注意：重启应用后新配置才会生效
    tracing::info!("所有配置已重置为默认值，请重启应用以应用更改");
    Ok(())
}

// ===== 开机自启动相关命令 =====

use tauri_plugin_autostart::ManagerExt;

/// 启用开机自启动
#[tauri::command]
async fn enable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    app.autolaunch()
        .enable()
        .map_err(|e: tauri_plugin_autostart::Error| e.to_string())?;
    tracing::info!("开机自启动已启用");
    Ok(())
}

/// 禁用开机自启动
#[tauri::command]
async fn disable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    app.autolaunch()
        .disable()
        .map_err(|e: tauri_plugin_autostart::Error| e.to_string())?;
    tracing::info!("开机自启动已禁用");
    Ok(())
}

/// 检查开机自启动状态
#[tauri::command]
async fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    let enabled = app.autolaunch()
        .is_enabled()
        .map_err(|e: tauri_plugin_autostart::Error| e.to_string())?;
    Ok(enabled)
}

/// 服务器配置 DTO
#[derive(serde::Serialize, serde::Deserialize)]
struct ServerConfigDto {
    bind_address: String,
    port: u16,
    max_connections: usize,
    /// 白名单是否启用
    whitelist_enabled: bool,
    /// 允许的客户端 IP 地址列表
    whitelist_addresses: Vec<String>,
}

/// 白名单配置 DTO
#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize)]
struct WhitelistConfigDto {
    enabled: bool,
    allowed_addresses: Vec<String>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 获取日志目录
    let log_dir = dirs::home_dir()
        .map(|home| home.join(".broadcast-service").join("logs"))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // 确保日志目录存在
    std::fs::create_dir_all(&log_dir).unwrap_or_else(|e| {
        eprintln!("无法创建日志目录 {:?}: {}", log_dir, e);
    });

    let log_path = log_dir.join("broadcast-manager.log");
    tracing::info!("日志目录: {:?}", log_dir);

    // 配置文件日志记录器（按天轮转）
    let file_appender = tracing_appender::rolling::daily(log_dir.clone(), "broadcast-manager");

    // 创建非阻塞写入器
    let (non_blocking_appender, _guard) = tracing_appender::non_blocking(file_appender);

    // 配置日志订阅器 - 同时输出到控制台和文件
    // 从配置读取日志级别，支持环境变量 RUST_LOG 覆盖
    let config = crate::config::Config::load().unwrap_or_default();
    let log_level: tracing::Level = config.server.log_level.into();
    let env_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(log_level.into());

    // 控制台层
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    // 文件层
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_appender)
        .with_ansi(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("广播服务管理器启动中...");
    tracing::info!("日志文件: {:?}", log_path);

    // 启动日志清理任务（保留最近 7 天的日志）
    let log_dir_clone = log_dir.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // 每小时检查一次
        loop {
            interval.tick().await;
            if let Ok(entries) = std::fs::read_dir(&log_dir_clone) {
                let now = std::time::SystemTime::now();
                let max_age = std::time::Duration::from_secs(7 * 24 * 3600); // 7 天

                for entry in entries.filter_map(Result::ok) {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    let path = entry.path();
                                    let path_str = path.to_string_lossy();
                                    // 只删除 .log 或 .log.* 文件
                                    if path.extension().map(|s| s.to_string_lossy()).unwrap_or_default() == "log" ||
                                       path_str.ends_with(".log") {
                                        let _ = std::fs::remove_file(&path);
                                        tracing::info!("清理过期日志文件: {:?}", path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // 保留 _guard 以防止文件日志过早关闭
    std::mem::forget(_guard);

    // 加载配置
    let config = Arc::new(Mutex::new(Config::load().unwrap_or_else(|e| {
        tracing::warn!("加载配置失败，使用默认配置: {}", e);
        Config::default()
    })));

    // 设置全局配置
    CONFIG.set(config.clone()).expect("Failed to set Config");

    // 启动音频 Actor
    let audio_actor = AudioActor::spawn(&config.lock().unwrap());
    AUDIO_ACTOR
        .set(audio_actor)
        .expect("Failed to set Audio Actor");

    tracing::info!("音频 Actor 已启动");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["broadcast-manager"]),
        ))
        .invoke_handler(tauri::generate_handler![
            get_audio_state,
            play_music,
            pause_music,
            stop_music,
            next_track,
            previous_track,
            play_track_at,
            set_music_volume,
            set_broadcast_volume,
            set_master_volume,
            start_ducking,
            stop_ducking,
            set_ducking_enabled,
            add_track,
            remove_track,
            clear_playlist,
            get_playlist,
            get_playback_progress,
            set_play_mode,
            scan_directory,
            set_ducking_volume,
            set_ducking_fade_in_ms,
            set_ducking_fade_out_ms,
            get_service_status,
            get_server_config,
            update_server_config,
            reset_all_config,
            enable_autostart,
            disable_autostart,
            is_autostart_enabled,
        ])
        .setup(move |_app| {
            tracing::info!("Tauri 应用初始化完成");

            // 启动内嵌网络服务（WebSocket + HTTP API）
            let config_clone = config.clone();

            tauri::async_runtime::spawn(async move {
                tracing::info!("启动内嵌广播服务...");
                // 克隆配置并释放锁，避免跨越 await 持有 MutexGuard
                let cfg = config_clone.lock().unwrap().clone();
                if let Err(e) = start_embedded_server(cfg).await {
                    tracing::error!("内嵌广播服务启动失败: {}", e);
                }
            });

            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // 关闭窗口时优雅关闭音频 Actor
                tracing::info!("正在关闭音频 Actor...");
                audio().shutdown();
                // 等待 Actor 线程结束
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 启动内嵌服务器
async fn start_embedded_server(
    config: Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use axum::Router;
    use server::http::{create_http_router, AppState};
    use server::websocket::create_ws_router;
    use tower_http::cors::{Any, CorsLayer};

    // 创建广播管理器
    let broadcast_manager = Arc::new(BroadcastManager::new(config.server.max_connections));

    // 创建应用状态
    let state = AppState { broadcast_manager };

    // 配置 CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 创建路由（合并 HTTP 和 WebSocket）
    let app = Router::new()
        .merge(create_http_router())
        .merge(create_ws_router())
        .layer(cors)
        .with_state(state);

    // 绑定地址
    let addr: SocketAddr = format!("{}:{}", config.server.bind_address, config.server.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("服务器启动在 http://{} (WebSocket: ws://{}/ws)", addr, addr);

    // 启动服务
    axum::serve(listener, app).await?;

    Ok(())
}
