use config::{Config as ConfigRs, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;

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
        // 从用户配置目录加载
        let config_dir = dirs::config_dir()
            .ok_or("无法获取配置目录")?;

        let app_config_dir = config_dir.join("broadcast-manager");
        let config_file = app_config_dir.join("config.toml");

        // 如果配置文件不存在，返回默认配置
        if !config_file.exists() {
            tracing::info!("配置文件不存在，使用默认配置");
            return Ok(Self::default());
        }

        // 读取配置文件内容
        let content = fs::read_to_string(&config_file)?;

        // 解析 TOML
        let config: Config = toml::from_str(&content)?;

        tracing::info!("配置已从 {:?} 加载", config_file);
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
        let config_dir = dirs::config_dir()
            .ok_or("无法获取配置目录")?;

        let app_config_dir = config_dir.join("broadcast-manager");
        fs::create_dir_all(&app_config_dir)?;

        let config_file = app_config_dir.join("config.toml");
        let toml_str = toml::to_string_pretty(self)?;

        let mut file = fs::File::create(config_file)?;
        file.write_all(toml_str.as_bytes())?;

        tracing::info!("配置已保存到 {:?}", app_config_dir);
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
