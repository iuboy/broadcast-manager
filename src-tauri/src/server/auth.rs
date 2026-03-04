//! API 访问控制模块
//!
//! 提供基于访问级别的 API 认证机制：
//! - Public: 公开访问，无需验证
//! - LocalOnly: 仅本地访问，检查来源 IP

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// API 访问级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AccessLevel {
    /// 公开访问，无需验证
    Public,
    /// 仅本地访问
    LocalOnly,
}

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AuthConfig {
    /// 本地 IP 地址列表（用于识别本地请求）
    pub local_addresses: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            local_addresses: vec![
                "127.0.0.1".to_string(),
                "::1".to_string(),
                "localhost".to_string(),
            ],
        }
    }
}

impl AuthConfig {
    /// 检查 IP 地址是否为本地地址
    pub fn is_local_address(&self, addr: &str) -> bool {
        // 解析 IP 地址
        let parsed: Option<IpAddr> = addr
            .parse()
            .ok()
            .or_else(|| {
                // 尝试从 socket 地址格式中提取 IP
                addr.split(':').next().and_then(|s| s.parse().ok())
            });

        if let Some(ip) = parsed {
            // 检查是否为回环地址
            if ip.is_loopback() {
                return true;
            }

            // 检查是否在本地地址列表中
            for local in &self.local_addresses {
                if addr.starts_with(local) || addr == local {
                    return true;
                }
            }
        }

        // 检查原始字符串
        self.local_addresses.iter().any(|local| {
            addr.starts_with(local) || addr == local || addr == format!("{}:*", local)
        })
    }
}

/// 根据路径获取访问级别
#[allow(dead_code)]
pub fn get_access_level(path: &str) -> AccessLevel {
    // WebSocket 端点 - 公开
    if path == "/ws" {
        return AccessLevel::Public;
    }

    // 健康检查和状态 - 公开
    if path == "/api/health" || path == "/api/status" {
        return AccessLevel::Public;
    }

    // 所有其他 /api/* 端点 - 仅本地访问
    if path.starts_with("/api/") {
        return AccessLevel::LocalOnly;
    }

    // 默认公开
    AccessLevel::Public
}

/// 验证请求是否满足访问级别要求
#[allow(dead_code)]
pub fn validate_request(
    req: &Request<Body>,
    auth_config: &AuthConfig,
    required_level: AccessLevel,
) -> Result<(), AuthError> {
    match required_level {
        AccessLevel::Public => Ok(()),
        AccessLevel::LocalOnly => {
            // 尝试从多个来源获取客户端地址
            let client_addr = get_client_address(req);

            match client_addr {
                Some(addr) => {
                    if auth_config.is_local_address(&addr) {
                        Ok(())
                    } else {
                        tracing::warn!(
                            "拒绝非本地访问: {} (path: {})",
                            addr,
                            req.uri().path()
                        );
                        Err(AuthError::LocalOnlyRequired)
                    }
                }
                None => {
                    // 如果无法确定客户端地址，默认允许（用于开发环境）
                    tracing::debug!("无法确定客户端地址，默认允许访问");
                    Ok(())
                }
            }
        }
    }
}

/// 从请求中获取客户端地址
#[allow(dead_code)]
fn get_client_address(req: &Request<Body>) -> Option<String> {
    // 1. 尝试从扩展中获取（由 Axum 的 ConnectInfo 添加）
    if let Some(addr) = req.extensions().get::<std::net::SocketAddr>() {
        return Some(addr.ip().to_string());
    }

    // 2. 尝试从 X-Forwarded-For 头获取
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // 取第一个 IP（原始客户端）
            if let Some(first_ip) = forwarded_str.split(',').next() {
                return Some(first_ip.trim().to_string());
            }
        }
    }

    // 3. 尝试从 X-Real-IP 头获取
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            return Some(ip_str.to_string());
        }
    }

    None
}

/// 认证错误
#[derive(Debug)]
#[allow(dead_code)]
pub enum AuthError {
    /// 需要 API Key
    ApiKeyRequired,
    /// API Key 无效
    InvalidApiKey,
    /// 需要本地访问
    LocalOnlyRequired,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::ApiKeyRequired => (StatusCode::UNAUTHORIZED, "需要 API Key"),
            AuthError::InvalidApiKey => (StatusCode::UNAUTHORIZED, "API Key 无效"),
            AuthError::LocalOnlyRequired => (StatusCode::FORBIDDEN, "仅允许本地访问"),
        };

        let body = serde_json::json!({
            "success": false,
            "message": message
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_local_address() {
        let config = AuthConfig::default();

        // 测试本地地址
        assert!(config.is_local_address("127.0.0.1"));
        assert!(config.is_local_address("127.0.0.1:8080"));
        assert!(config.is_local_address("::1"));
        assert!(config.is_local_address("localhost"));
        assert!(config.is_local_address("localhost:8080"));

        // 测试非本地地址
        assert!(!config.is_local_address("192.168.1.1"));
        assert!(!config.is_local_address("10.0.0.1"));
    }

    #[test]
    fn test_get_access_level() {
        assert_eq!(get_access_level("/ws"), AccessLevel::Public);
        assert_eq!(get_access_level("/api/health"), AccessLevel::Public);
        assert_eq!(get_access_level("/api/status"), AccessLevel::Public);
        assert_eq!(get_access_level("/api/playlist"), AccessLevel::LocalOnly);
        assert_eq!(get_access_level("/api/control/play"), AccessLevel::LocalOnly);
    }
}
