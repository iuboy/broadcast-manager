//! HTTP API 路由
//!
//! 提供简化的 RESTful API，只保留健康检查端点
//! 所有音频操作已迁移到 Tauri 命令

use axum::{routing::get, Json, Router};
use serde_json::Value;
use std::sync::Arc;

use crate::server::BroadcastManager;

/// 统一应用状态
#[derive(Clone)]
pub struct AppState {
    pub broadcast_manager: Arc<BroadcastManager>,
}

/// 创建 HTTP 路由（仅保留健康检查）
pub fn create_http_router() -> Router<AppState> {
    Router::new().route("/api/health", get(health_check))
}

// ===== 状态 API =====

/// 健康检查端点
async fn health_check() -> Json<Value> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Json(serde_json::json!({
        "status": "ok",
        "timestamp": timestamp
    }))
}
