//! WebSocket 处理器
//!
//! 处理客户端的 WebSocket 连接，接收音频数据并通过 Audio Actor 处理

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::time::{Duration, Instant};

use super::http::AppState;

/// 心跳间隔（秒）
const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// 心跳超时（秒）
const HEARTBEAT_TIMEOUT_SECS: u64 = 60;

/// WebSocket URL 参数
#[derive(Debug, Deserialize)]
pub struct WsParams {
    /// 编解码格式（pcm/opus），默认 pcm
    #[serde(default = "default_codec")]
    pub codec: String,
    /// 采样率（8000-48000），默认 44100
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// 声道数（1=单声道，2=立体声），默认 1
    #[serde(default = "default_channels")]
    pub channels: u16,
}

fn default_codec() -> String {
    "pcm".to_string()
}

fn default_sample_rate() -> u32 {
    44100  // 改为 44100Hz 以兼容大多数设备
}

fn default_channels() -> u16 {
    1
}

impl Default for WsParams {
    fn default() -> Self {
        Self {
            codec: default_codec(),
            sample_rate: default_sample_rate(),
            channels: default_channels(),
        }
    }
}

impl WsParams {
    /// 验证参数有效性
    pub fn validate(&self) -> Result<(), String> {
        // 验证编解码格式
        if !matches!(self.codec.to_lowercase().as_str(), "pcm" | "opus") {
            return Err(format!("不支持的编解码格式: {}，支持: pcm, opus", self.codec));
        }

        // 验证采样率
        if ![8000, 16000, 24000, 44100, 48000].contains(&self.sample_rate) {
            return Err(format!(
                "不支持的采样率: {}，支持: 8000, 16000, 24000, 44100, 48000",
                self.sample_rate
            ));
        }

        // 验证声道数
        if ![1, 2].contains(&self.channels) {
            return Err(format!("不支持的声道数: {}，支持: 1（单声道）, 2（立体声）", self.channels));
        }

        Ok(())
    }
}

// ===== WebSocket 消息协议 =====

/// 发送就绪消息
fn send_ready() -> Message {
    Message::Text("ready".to_string())
}

/// 发送错误消息
fn send_error(msg: &str) -> Message {
    Message::Text(format!("error:{}", msg))
}

/// WebSocket 升级处理
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> Response {
    // 验证参数
    if let Err(e) = params.validate() {
        tracing::warn!("WebSocket 连接参数无效: {}", e);
        // 返回错误响应（在 WebSocket 升级前）
        return Response::builder()
            .status(400)
            .body(format!("参数错误: {}", e).into())
            .unwrap();
    }

    tracing::info!(
        "WebSocket 连接请求: codec={}, sample_rate={}, channels={}",
        params.codec,
        params.sample_rate,
        params.channels
    );

    ws.on_upgrade(move |socket| handle_socket(socket, state, params))
}

/// 处理 WebSocket 连接
async fn handle_socket(socket: WebSocket, state: AppState, params: WsParams) {
    // 添加客户端
    let client_id = match state.broadcast_manager.add_client() {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("无法添加客户端: {}", e);
            let _ = socket.close().await;
            return;
        }
    };

    tracing::info!(
        "WebSocket 客户端连接: {} (codec={}, {}Hz, {}ch)",
        client_id,
        params.codec,
        params.sample_rate,
        params.channels
    );

    let (mut sender, mut receiver) = socket.split();

    // 发送就绪消息
    tracing::info!("正在发送就绪消息到客户端 {}", client_id);
    match sender.send(send_ready()).await {
        Ok(()) => tracing::info!("就绪消息发送成功"),
        Err(e) => {
            tracing::warn!("无法发送就绪消息: {}", e);
            state.broadcast_manager.remove_client(&client_id);
            return;
        }
    }

    // 心跳状态
    let mut last_heartbeat = Instant::now();
    let mut is_broadcasting = false;

    // 消息处理循环
    tracing::info!("客户端 {} 进入消息循环，将在 {} 秒后检查超时", client_id, HEARTBEAT_INTERVAL_SECS);

    // 添加调试：检查 receiver 是否正常工作
    tracing::info!("客户端 {} receiver 准备就绪", client_id);

    loop {
        // 使用 timeout 确保超时检查能够执行
        tracing::debug!("客户端 {} 等待下一条消息...", client_id);
        let msg = tokio::time::timeout(
            Duration::from_secs(HEARTBEAT_INTERVAL_SECS),
            receiver.next()
        ).await;

        tracing::debug!("客户端 {} 收到事件: {:?}", client_id, msg);

        match msg {
            Ok(Some(Ok(msg))) => {
                last_heartbeat = Instant::now();

                match msg {
                    // 文本消息（控制命令）
                    Message::Text(text) => {
                        tracing::info!("客户端 {} 收到文本消息: '{}'", client_id, text);
                        match text.as_str() {
                            "start_broadcast" => {
                                // 请求开始广播
                                tracing::info!("客户端 {} 请求开始广播", client_id);

                                // 注意：当前客户端只发送 PCM 数据，无论 codec 参数是什么
                                // 为了确保兼容性，我们强制使用 PCM 解码
                                let actual_codec = if params.codec.to_lowercase() == "opus" {
                                    tracing::warn!("客户端请求 Opus 编解码，但当前实现只支持 PCM。将使用 PCM 模式。");
                                    "pcm".to_string()
                                } else {
                                    params.codec.clone()
                                };

                                // 使用客户端请求的采样率（这样服务端和客户端完全匹配）
                                tracing::info!(
                                    "使用客户端采样率: {}Hz",
                                    params.sample_rate
                                );

                                // 根据客户端采样率重新配置广播引擎
                                crate::audio().reconfigure_broadcast(
                                    actual_codec,
                                    params.sample_rate,  // 使用客户端请求的采样率
                                    params.channels
                                );

                                match state.broadcast_manager.request_broadcast(&client_id) {
                                    Ok(()) => {
                                        is_broadcasting = true;
                                        // 通知 Audio Actor 开始闪避
                                        crate::audio().start_ducking();
                                        tracing::info!("客户端 {} 开始广播，发送 broadcasting 响应", client_id);
                                        match sender.send(Message::Text("broadcasting".to_string())).await {
                                            Ok(()) => tracing::info!("broadcasting 响应发送成功"),
                                            Err(e) => tracing::error!("broadcasting 响应发送失败: {}", e),
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("客户端 {} 请求广播失败: {}", client_id, e);
                                        let _ = sender.send(send_error(&e)).await;
                                    }
                                }
                            }
                            "stop_broadcast" => {
                                tracing::warn!("==================== STOP_BROADCAST 开始 ====================");
                                // 结束广播
                                state.broadcast_manager.end_broadcast(&client_id);
                                is_broadcasting = false;
                                // 通知 Audio Actor 停止闪避
                                tracing::warn!("客户端 {} 请求停止闪避", client_id);
                                crate::audio().stop_ducking();
                                tracing::warn!("客户端 {} 已发送停止闪避请求", client_id);
                                let _ = sender.send(Message::Text("idle".to_string())).await;
                                tracing::warn!("==================== STOP_BROADCAST 结束 ====================");
                            }
                            "heartbeat" => {
                                // 心跳响应
                                state.broadcast_manager.update_activity(&client_id);
                                let _ = sender.send(Message::Text("pong".to_string())).await;
                            }
                            _ => {
                                tracing::debug!("收到未知文本消息: {}", text);
                            }
                        }
                    }
                    // 二进制消息（音频数据）
                    Message::Binary(data) => {
                        tracing::debug!("客户端 {} 收到二进制数据: {} 字节, is_broadcasting={}",
                            client_id, data.len(), is_broadcasting);
                        if is_broadcasting {
                            // 通过 Audio Actor 推送音频数据
                            crate::audio().push_broadcast_bytes(data);
                        } else {
                            tracing::warn!("客户端 {} 发送音频数据但未在广播状态，丢弃 {} 字节",
                                client_id, data.len());
                        }
                    }
                    Message::Ping(data) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    Message::Pong(_) => {}
                    Message::Close(_) => {
                        tracing::info!("客户端 {} 关闭连接", client_id);
                        break;
                    }
                }
            }
            Ok(Some(Err(e))) => {
                tracing::warn!("客户端 {} WebSocket 错误: {}", client_id, e);
                break;
            }
            Ok(None) => {
                tracing::info!("客户端 {} 连接已关闭（None）", client_id);
                break;
            }
            Err(_) => {
                // 超时 - 检查心跳超时
                let elapsed = last_heartbeat.elapsed().as_secs();
                tracing::debug!("客户端 {} 心跳检查: 上次活动 {} 秒前", client_id, elapsed);
                if elapsed > HEARTBEAT_TIMEOUT_SECS as u64 {
                    tracing::warn!("客户端 {} 心跳超时 ({} 秒无活动)", client_id, elapsed);
                    break;
                }
                // 如果还没超时，继续循环
            }
        }
    }

    // 清理 - 总是尝试停止闪避，确保状态一致
    let was_broadcasting = state.broadcast_manager.is_broadcasting() &&
                          state.broadcast_manager.current_broadcaster().as_deref() == Some(&client_id);

    if was_broadcasting {
        tracing::info!("客户端 {} 断开，停止广播和闪避", client_id);
        state.broadcast_manager.end_broadcast(&client_id);
        crate::audio().stop_ducking();
    }

    state.broadcast_manager.remove_client(&client_id);
    tracing::info!("客户端 {} 断开连接", client_id);
}

/// 创建 WebSocket 路由
pub fn create_ws_router() -> Router<AppState> {
    Router::new().route("/ws", get(ws_handler))
}
