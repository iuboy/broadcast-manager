use config::{Config as ConfigRs, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;



/// 日志级别配置
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevelConfig {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevelConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl From<LogLevelConfig> for tracing::Level {
    fn from(level: LogLevelConfig) -> Self {
        match level {
            LogLevelConfig::Trace => Self::TRACE,
            LogLevelConfig::Debug => Self::DEBUG,
            LogLevelConfig::Info => Self::INFO,
            LogLevelConfig::Warn => Self::WARN,
            LogLevelConfig::Error => Self::ERROR,
        }
    }
}

/// 认证配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthConfig {
    /// 是否启用本地访问限制
    #[serde(default = "default_auth_enabled")]
    pub enabled: bool,
    /// 本地 IP 地址列表
    #[serde(default = "default_local_addresses")]
    pub local_addresses: Vec<String>,
}

/// 客户端白名单配置（用于控制哪些客户端可以发起广播请求）
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ClientWhitelistConfig {
    /// 是否启用白名单检查
    #[serde(default)]
    pub enabled: bool,
    /// 允许发起广播请求的客户端 IP 地址列表
    /// 环回地址（127.0.0.1, ::1, localhost）总是被允许，无需显式列出
    #[serde(default)]
    pub allowed_addresses: Vec<String>,
}

fn default_auth_enabled() -> bool {
    true
}

fn default_local_addresses() -> Vec<String> {
    vec![
        "127.0.0.1".to_string(),
        "::1".to_string(),
        "localhost".to_string(),
    ]
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            local_addresses: default_local_addresses(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    /// 监听地址 (例如: 0.0.0.0, 127.0.0.1, localhost)
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// 服务端口 (WebSocket 和 HTTP API 共用)
    #[serde(default = "default_port")]
    pub port: u16,
    pub max_connections: usize,
    #[serde(default)]
    pub cors: CorsConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    /// 客户端白名单配置（用于控制广播请求）
    #[serde(default)]
    pub client_whitelist: ClientWhitelistConfig,
    /// 日志级别配置
    #[serde(default)]
    pub log_level: LogLevelConfig,
}

fn default_port() -> u16 {
    8081
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

/// CORS 配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CorsConfig {
    /// 允许的来源
    pub allowed_origins: Vec<String>,
    /// 允许的 HTTP 方法
    pub allowed_methods: Option<Vec<String>>,
    /// 允许的请求头
    pub allowed_headers: Option<Vec<String>>,
    /// 是否允许携带凭证
    pub allow_credentials: Option<bool>,
    /// 预检请求缓存时间（秒）
    pub max_age: Option<u64>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        // 安全警告：允许所有来源的跨域请求（*）对于桌面应用风险较低，
        // 因为应用运行在用户信任的环境中。如果需要在公网部署服务，
        // 建议将 allowed_origins 修改为具体的域名列表。
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: None,
            allowed_headers: None,
            allow_credentials: None,
            max_age: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DuckingConfig {
    pub enabled: bool,
    pub volume: f32,
    pub fade_in_ms: u64,
    pub fade_out_ms: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MusicConfig {
    pub music_dir: String,
    pub volume: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BroadcastConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: String,
}

impl Default for BroadcastConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,  // 改为 44100Hz 以兼容大多数设备
            channels: 1,
            codec: "pcm".to_string(),
        }
    }
}

/// 运行时配置（会随着用户操作改变）
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RuntimeConfig {
    /// 主音量
    pub master_volume: f32,
    /// 音乐音量
    pub music_volume: f32,
    /// 广播音量
    pub broadcast_volume: f32,
    /// 闪避是否启用
    pub ducking_enabled: bool,
    /// 闪避音量
    pub ducking_volume: f32,
    /// 闪避淡入时间（毫秒）
    pub ducking_fade_in_ms: u64,
    /// 闪避淡出时间（毫秒）
    pub ducking_fade_out_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 1.0,
            broadcast_volume: 1.0,
            ducking_enabled: true,
            ducking_volume: 0.3,
            ducking_fade_in_ms: 500,
            ducking_fade_out_ms: 1000,
        }
    }
}

/// 应用配置（重命名为 Config 以简化使用）
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub ducking: DuckingConfig,
    pub music: MusicConfig,
    #[serde(default)]
    pub broadcast: BroadcastConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    /// 播放列表（存储文件路径）
    #[serde(default)]
    pub playlist: Vec<String>,
}

impl Config {
    /// 从文件加载配置
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // 使用统一的配置目录 ~/.broadcast-service/
        let config_dir = dirs::home_dir()
            .map(|home| home.join(".broadcast-service"))
            .unwrap_or_else(|| std::path::PathBuf::from(".broadcast-service"));

        let config_file = config_dir.join("broadcast-manager-config.toml");

        // 如果配置文件不存在，返回默认配置
        if !config_file.exists() {
            tracing::info!("配置文件不存在，使用默认配置");
            return Ok(Self::default());
        }

        // 读取配置文件内容
        let content = fs::read_to_string(&config_file)?;

        // 解析 TOML
        let config: Config = toml::from_str(&content)?;

        tracing::info!("配置已从 {:?} 加载", &config_file);
        Ok(config)
    }

    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let config = ConfigRs::builder()
            .add_source(File::with_name(path))
            .build()?;
        config.try_deserialize()
    }

    /// 保存配置到文件
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        // 使用统一的配置目录 ~/.broadcast-service/
        let config_dir = dirs::home_dir()
            .map(|home| home.join(".broadcast-service"))
            .unwrap_or_else(|| std::path::PathBuf::from(".broadcast-service"));

        fs::create_dir_all(&config_dir)?;

        // 设置目录权限为 0700（仅所有者可访问）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(mut perms) = std::fs::metadata(&config_dir).map(|m| m.permissions()) {
                perms.set_mode(0o700);
                let _ = std::fs::set_permissions(&config_dir, perms);
            }
        }

        let config_file = config_dir.join("broadcast-manager-config.toml");
        let toml_str = toml::to_string_pretty(self)?;

        let mut file = fs::File::create(&config_file)?;
        file.write_all(toml_str.as_bytes())?;

        // 设置文件权限为 0600（仅所有者可读写）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(mut perms) = std::fs::metadata(&config_file).map(|m| m.permissions()) {
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&config_file, perms);
            }
        }

        tracing::info!("配置已保存到 {:?}", &config_file);
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_address: "0.0.0.0".to_string(),
                port: 8081,
                max_connections: 10,
                cors: CorsConfig::default(),
                auth: AuthConfig::default(),
                client_whitelist: ClientWhitelistConfig::default(),
                log_level: LogLevelConfig::default(),
            },
            ducking: DuckingConfig {
                enabled: true,
                volume: 0.3,
                fade_in_ms: 400,
                fade_out_ms: 600,
            },
            music: MusicConfig {
                music_dir: "./music".to_string(),
                volume: 0.7,
            },
            broadcast: BroadcastConfig::default(),
            runtime: RuntimeConfig::default(),
            playlist: Vec::new(),
        }
    }
}
