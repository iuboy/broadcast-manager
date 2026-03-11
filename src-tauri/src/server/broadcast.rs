//! # 广播管理器模块
//!
//! 管理广播状态和客户端连接。
//!
//! ## 功能
//! - 客户端连接管理
//! - 广播状态控制（空闲/广播中）
//! - 广播者唯一性保证
//! - 客户端活动跟踪
//!
//! ## 广播流程
//! 1. 客户端连接 → `add_client()`
//! 2. 请求广播 → `request_broadcast()`
//! 3. 服务端检查是否已有广播者
//! 4. 如果空闲，批准请求并更新状态
//! 5. 广播结束或断开 → `end_broadcast()` / `remove_client()`
//!
//! ## 注意
//! 不支持断线重连，客户端断开后广播状态会立即清理。

use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

/// 广播客户端信息
#[derive(Debug, Clone)]
pub struct BroadcastClient {
    #[allow(dead_code)]
    pub id: String,
    // connected_at 字段保留用于调试和监控
    #[allow(dead_code)]
    pub connected_at: Instant,
    pub last_activity: Instant,
}

/// 广播状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastState {
    Idle,
    Broadcasting,
}

use serde::{Deserialize, Serialize};

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
        let now = Instant::now();

        clients.insert(
            id.clone(),
            BroadcastClient {
                id: id.clone(),
                connected_at: now,
                last_activity: now,
            },
        );

        tracing::info!("客户端连接: {}, 当前连接数: {}", id, clients.len());
        Ok(id)
    }

    /// 移除客户端
    pub fn remove_client(&self, id: &str) {
        let mut clients = self.clients.write();
        if clients.remove(id).is_some() {
            tracing::info!("客户端断开: {}, 剩余连接数: {}", id, clients.len());
        }
        // 注意：广播状态的清理由 websocket.rs 的 handle_socket 统一处理
    }

    /// 更新客户端活动时间
    pub fn update_activity(&self, id: &str) {
        let mut clients = self.clients.write();
        if let Some(client) = clients.get_mut(id) {
            client.last_activity = Instant::now();
        }
    }

    /// 请求开始广播
    ///
    /// 使用单一写锁进行原子操作，防止 TOCTOU 竞态条件。
    pub fn request_broadcast(&self, client_id: &str) -> Result<(), String> {
        // 使用写锁进行原子操作，避免多次锁释放导致的竞态窗口
        let mut state = self.state.write();
        let mut broadcaster = self.current_broadcaster.write();
        let mut broadcast_start = self.broadcast_start.write();
        let clients = self.clients.read();

        // 检查是否正在广播
        if *state == BroadcastState::Broadcasting {
            return Err(format!(
                "已有客户端正在广播: {}",
                broadcaster.as_deref().unwrap_or("unknown")
            ));
        }

        // 验证客户端存在
        if !clients.contains_key(client_id) {
            return Err("客户端不存在".to_string());
        }

        // 开始广播（在同一原子操作中更新所有状态）
        *state = BroadcastState::Broadcasting;
        *broadcaster = Some(client_id.to_string());
        *broadcast_start = Some(Instant::now());

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
    #[allow(dead_code)]
    pub fn is_broadcasting(&self) -> bool {
        *self.state.read() == BroadcastState::Broadcasting
    }

    /// 获取当前广播者
    #[allow(dead_code)]
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
        self.current_broadcaster.read().as_deref() == Some(client_id)
    }

    /// 原子检查：客户端是否正在广播（防止竞态条件）
    pub fn is_broadcasting_and_broadcaster(&self, client_id: &str) -> bool {
        let broadcaster = self.current_broadcaster.read();
        broadcaster.as_deref() == Some(client_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_manager_new() {
        let manager = BroadcastManager::new(5);
        assert_eq!(manager.client_count(), 0);
        assert!(!manager.is_broadcasting());
        assert_eq!(manager.state(), BroadcastState::Idle);
    }

    #[test]
    fn test_add_client() {
        let manager = BroadcastManager::new(2);

        let id1 = manager.add_client();
        assert!(id1.is_ok());
        assert_eq!(manager.client_count(), 1);

        let id2 = manager.add_client();
        assert!(id2.is_ok());
        assert_eq!(manager.client_count(), 2);
    }

    #[test]
    fn test_max_connections() {
        let manager = BroadcastManager::new(2);

        let _id1 = manager.add_client().unwrap();
        let _id2 = manager.add_client().unwrap();

        let id3 = manager.add_client();
        assert!(id3.is_err());
        assert!(id3.unwrap_err().contains("最大连接数"));
    }

    #[test]
    fn test_remove_client() {
        let manager = BroadcastManager::new(10);

        let id = manager.add_client().unwrap();
        assert_eq!(manager.client_count(), 1);
        assert!(manager.has_client(&id));

        manager.remove_client(&id);
        assert_eq!(manager.client_count(), 0);
        assert!(!manager.has_client(&id));
    }

    #[test]
    fn test_request_broadcast_success() {
        let manager = BroadcastManager::new(10);
        let id = manager.add_client().unwrap();

        let result = manager.request_broadcast(&id);
        assert!(result.is_ok());
        assert!(manager.is_broadcasting());
        assert_eq!(manager.current_broadcaster(), Some(id));
    }

    #[test]
    fn test_request_broadcast_already_broadcasting() {
        let manager = BroadcastManager::new(10);
        let id1 = manager.add_client().unwrap();
        let id2 = manager.add_client().unwrap();

        manager.request_broadcast(&id1).unwrap();

        let result = manager.request_broadcast(&id2);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已有客户端正在广播"));
    }

    #[test]
    fn test_request_broadcast_nonexistent_client() {
        let manager = BroadcastManager::new(10);

        let result = manager.request_broadcast("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("客户端不存在"));
    }

    #[test]
    fn test_end_broadcast() {
        let manager = BroadcastManager::new(10);
        let id = manager.add_client().unwrap();

        manager.request_broadcast(&id).unwrap();
        assert!(manager.is_broadcasting());

        manager.end_broadcast(&id);
        assert!(!manager.is_broadcasting());
        assert_eq!(manager.current_broadcaster(), None);
    }

    #[test]
    fn test_is_broadcaster() {
        let manager = BroadcastManager::new(10);
        let id1 = manager.add_client().unwrap();
        let id2 = manager.add_client().unwrap();

        manager.request_broadcast(&id1).unwrap();

        assert!(manager.is_broadcaster(&id1));
        assert!(!manager.is_broadcaster(&id2));
    }

    #[test]
    fn test_update_activity() {
        let manager = BroadcastManager::new(10);
        let id = manager.add_client().unwrap();

        // This test mainly verifies the method exists and doesn't panic
        manager.update_activity(&id);
        assert!(manager.has_client(&id));
    }

    #[test]
    fn test_get_client_ids() {
        let manager = BroadcastManager::new(10);

        let id1 = manager.add_client().unwrap();
        let id2 = manager.add_client().unwrap();

        let ids = manager.get_client_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_broadcast_duration() {
        let manager = BroadcastManager::new(10);
        let id = manager.add_client().unwrap();

        // Initially no duration
        assert!(manager.broadcast_duration_secs().is_none());

        manager.request_broadcast(&id).unwrap();

        // After broadcast starts, we should have a duration
        let duration = manager.broadcast_duration_secs();
        assert!(duration.is_some());
        assert!(duration.unwrap() >= 0.0);
    }

    #[test]
    fn test_default() {
        let manager = BroadcastManager::default();
        assert_eq!(manager.client_count(), 0);
        assert!(!manager.is_broadcasting());
    }

    #[test]
    fn test_multiple_broadcasts_sequential() {
        let manager = BroadcastManager::new(10);
        let id1 = manager.add_client().unwrap();
        let id2 = manager.add_client().unwrap();

        // First broadcast
        manager.request_broadcast(&id1).unwrap();
        assert_eq!(manager.current_broadcaster(), Some(id1.clone()));

        // End first broadcast
        manager.end_broadcast(&id1);
        assert_eq!(manager.current_broadcaster(), None);

        // Second broadcast
        manager.request_broadcast(&id2).unwrap();
        assert_eq!(manager.current_broadcaster(), Some(id2));
    }
}
