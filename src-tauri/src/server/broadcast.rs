use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// 会话超时时间（秒）
#[allow(dead_code)]
const SESSION_TIMEOUT_SECS: u64 = 300; // 5 分钟

/// 广播客户端信息
#[derive(Debug, Clone)]
pub struct BroadcastClient {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub session_token: String,
}

/// 广播状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastState {
    Idle,
    Broadcasting,
}

use serde::{Deserialize, Serialize};

/// 会话信息（用于重连）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub client_id: String,
    pub session_token: String,
    pub was_broadcaster: bool,
}

/// 广播管理器
pub struct BroadcastManager {
    /// 当前状态
    state: RwLock<BroadcastState>,
    /// 当前广播客户端
    current_broadcaster: RwLock<Option<String>>,
    /// 广播开始时间
    broadcast_start: RwLock<Option<Instant>>,
    /// 所有连接的客户端
    clients: RwLock<HashMap<String, BroadcastClient>>,
    /// 断开客户端的会话缓存（用于重连）
    session_cache: RwLock<HashMap<String, (SessionInfo, Instant)>>,
    /// 最大连接数
    max_connections: usize,
}

impl BroadcastManager {
    pub fn new(max_connections: usize) -> Self {
        Self {
            state: RwLock::new(BroadcastState::Idle),
            current_broadcaster: RwLock::new(None),
            broadcast_start: RwLock::new(None),
            clients: RwLock::new(HashMap::new()),
            session_cache: RwLock::new(HashMap::new()),
            max_connections,
        }
    }

    /// 添加新客户端
    pub fn add_client(&self) -> Result<String, String> {
        let mut clients = self.clients.write();
        if clients.len() >= self.max_connections {
            return Err("已达到最大连接数".to_string());
        }

        let id = Uuid::new_v4().to_string();
        let session_token = Uuid::new_v4().to_string();
        let now = Instant::now();

        clients.insert(
            id.clone(),
            BroadcastClient {
                id: id.clone(),
                connected_at: now,
                last_activity: now,
                session_token: session_token.clone(),
            },
        );

        tracing::info!("客户端连接: {}, 当前连接数: {}", id, clients.len());
        Ok(id)
    }

    /// 客户端重连（使用之前的会话）
    #[allow(dead_code)]
    pub fn reconnect_client(&self, session_token: &str) -> Result<String, String> {
        // 清理过期会话
        self.cleanup_expired_sessions();

        // 查找会话缓存
        let mut cache = self.session_cache.write();
        if let Some((session_info, _)) = cache.remove(session_token) {
            let mut clients = self.clients.write();
            if clients.len() >= self.max_connections {
                return Err("已达到最大连接数".to_string());
            }

            let client_id = session_info.client_id.clone();
            let new_session_token = Uuid::new_v4().to_string();
            let now = Instant::now();

            clients.insert(
                client_id.clone(),
                BroadcastClient {
                    id: client_id.clone(),
                    connected_at: now,
                    last_activity: now,
                    session_token: new_session_token.clone(),
                },
            );

            // 如果之前是广播者，自动恢复广播
            if session_info.was_broadcaster {
                *self.state.write() = BroadcastState::Broadcasting;
                *self.current_broadcaster.write() = Some(client_id.clone());
                *self.broadcast_start.write() = Some(Instant::now());
                tracing::info!("客户端 {} 重连并恢复广播", client_id);
            }

            tracing::info!(
                "客户端重连: {}, 当前连接数: {}",
                client_id,
                clients.len()
            );
            return Ok(client_id);
        }

        Err("无效或过期的会话令牌".to_string())
    }

    /// 移除客户端
    pub fn remove_client(&self, id: &str) {
        let mut clients = self.clients.write();
        if let Some(client) = clients.remove(id) {
            tracing::info!("客户端断开: {}, 剩余连接数: {}", id, clients.len());

            // 检查是否是当前广播者
            let was_broadcaster = self.is_broadcaster(id);

            // 保存会话到缓存（允许重连）
            let session_info = SessionInfo {
                client_id: id.to_string(),
                session_token: client.session_token.clone(),
                was_broadcaster,
            };
            self.session_cache
                .write()
                .insert(client.session_token, (session_info, Instant::now()));

            // 如果是当前广播者断开，不立即清理状态（允许重连）
            if was_broadcaster {
                tracing::info!("广播者断开连接，等待重连...");
                // 设置一个短暂的等待时间，不立即清除广播状态
                // 实际的重连逻辑由客户端在超时前发起
            }
        }

        // 如果是当前广播者断开，清理状态
        {
            let mut broadcaster = self.current_broadcaster.write();
            if broadcaster.as_deref() == Some(id) {
                *broadcaster = None;
                *self.state.write() = BroadcastState::Idle;
                tracing::info!("广播者断开连接，广播结束");
            }
        }
    }

    /// 更新客户端活动时间
    pub fn update_activity(&self, id: &str) {
        let mut clients = self.clients.write();
        if let Some(client) = clients.get_mut(id) {
            client.last_activity = Instant::now();
        }
    }

    /// 清理过期会话
    #[allow(dead_code)]
    pub fn cleanup_expired_sessions(&self) {
        let mut cache = self.session_cache.write();
        let now = Instant::now();
        let timeout = Duration::from_secs(SESSION_TIMEOUT_SECS);

        cache.retain(|_, (_, disconnected_at)| {
            now.duration_since(*disconnected_at) < timeout
        });
    }

    /// 获取会话令牌（用于客户端保存以便重连）
    #[allow(dead_code)]
    pub fn get_session_token(&self, client_id: &str) -> Option<String> {
        let clients = self.clients.read();
        clients.get(client_id).map(|c| c.session_token.clone())
    }

    /// 请求开始广播
    pub fn request_broadcast(&self, client_id: &str) -> Result<(), String> {
        // 先检查是否正在广播
        {
            let state = self.state.read();
            if *state == BroadcastState::Broadcasting {
                let broadcaster = self.current_broadcaster.read();
                return Err(format!(
                    "已有客户端正在广播: {}",
                    broadcaster.as_deref().unwrap_or("unknown")
                ));
            }
        } // 读锁在这里释放

        // 验证客户端存在
        {
            let clients = self.clients.read();
            if !clients.contains_key(client_id) {
                return Err("客户端不存在".to_string());
            }
        } // 读锁在这里释放

        // 开始广播（获取写锁）
        *self.state.write() = BroadcastState::Broadcasting;
        *self.current_broadcaster.write() = Some(client_id.to_string());
        *self.broadcast_start.write() = Some(Instant::now());

        tracing::info!("客户端 {} 开始广播", client_id);
        Ok(())
    }

    /// 结束广播
    pub fn end_broadcast(&self, client_id: &str) {
        // 先检查是否是广播者（使用读锁）
        let is_broadcaster = self.is_broadcaster(client_id);

        if is_broadcaster {
            // 释放读锁后再获取写锁
            *self.state.write() = BroadcastState::Idle;
            *self.current_broadcaster.write() = None;
            *self.broadcast_start.write() = None;
            tracing::info!("客户端 {} 结束广播", client_id);
        }
    }

    /// 获取当前状态
    #[allow(dead_code)]
    pub fn state(&self) -> BroadcastState {
        *self.state.read()
    }

    /// 是否正在广播
    pub fn is_broadcasting(&self) -> bool {
        *self.state.read() == BroadcastState::Broadcasting
    }

    /// 获取当前广播者
    pub fn current_broadcaster(&self) -> Option<String> {
        self.current_broadcaster.read().clone()
    }

    /// 获取广播持续时间（秒）
    #[allow(dead_code)]
    pub fn broadcast_duration_secs(&self) -> Option<f64> {
        let start = self.broadcast_start.read();
        start.map(|s| s.elapsed().as_secs_f64())
    }

    /// 获取客户端数量
    #[allow(dead_code)]
    pub fn client_count(&self) -> usize {
        self.clients.read().len()
    }

    /// 验证客户端是否是当前广播者
    pub fn is_broadcaster(&self, client_id: &str) -> bool {
        self.current_broadcaster
            .read()
            .as_deref()
            == Some(client_id)
    }

    /// 获取所有客户端 ID
    #[allow(dead_code)]
    pub fn get_client_ids(&self) -> Vec<String> {
        self.clients.read().keys().cloned().collect()
    }

    /// 检查客户端是否存在
    #[allow(dead_code)]
    pub fn has_client(&self, client_id: &str) -> bool {
        self.clients.read().contains_key(client_id)
    }
}

impl Default for BroadcastManager {
    fn default() -> Self {
        Self::new(10)
    }
}
