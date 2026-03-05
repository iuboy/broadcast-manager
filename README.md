# Broadcast Manager

全功能桌面广播管理应用，基于 Tauri + Vue 3 + Rust 构建。

## 功能特性

- 🎵 **实时音频广播** - WebSocket 低延迟音频传输
- 🔊 **智能混音** - 自动闪避（Ducking），广播时自动降低背景音乐音量
- 🎛️ **播放列表管理** - 支持 MP3、WAV、FLAC 等多种格式
- 📡 **多客户端支持** - 支持多个监听客户端同时连接
- 🔒 **安全防护** - IP 白名单、速率限制、心跳保活
- 📊 **状态监控** - 实时查看广播状态和客户端信息
- 🚀 **高性能** - Rust 后端 + Actor 模式音频处理

## 快速开始

### 环境要求

- Node.js >= 24.0
- pnpm >= 10.0
- Rust >= 1.80

### 安装依赖

```bash
# 安装前端依赖
pnpm install

# Linux 系统需要安装额外的系统库
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libopus-dev libasound2-dev
```

### 开发模式

```bash
pnpm tauri dev
```

### 生产构建

```bash
pnpm build
```

## 文档

- [部署指南](./docs/DEPLOYMENT.md) - 生产环境部署配置
- [开发指南](./docs/DEVELOPMENT.md) - 参与开发的相关信息
- [API 文档](./docs/openapi.yaml) - WebSocket 和 HTTP API 规范

## 配置

配置文件位置：`~/.broadcast-service/broadcast-manager-config.toml`

```toml
[server]
bind_address = "0.0.0.0"
port = 8081
max_connections = 10

[ducking]
enabled = true
volume = 0.3
fade_in_ms = 400
fade_out_ms = 600

[broadcast]
sample_rate = 44100
channels = 1
codec = "pcm"
```

详细配置说明请参考 [部署指南](./docs/DEPLOYMENT.md#配置)。

## 技术栈

### 前端
- **Vue 3** - 渐进式 JavaScript 框架
- **TypeScript** - 类型安全
- **Element Plus** - UI 组件库
- **Pinia** - 状态管理
- **Vite** - 构建工具

### 后端
- **Tauri 2** - 桌面应用框架
- **Rust** - 系统编程语言
- **Axum** - Web 框架
- **Tokio** - 异步运行时
- **Rodio/Symphonia** - 音频处理

## WebSocket API

### 连接

```javascript
const ws = new WebSocket('ws://localhost:8081/ws?codec=pcm&sample_rate=44100&channels=1');
```

### 消息协议

**客户端 → 服务端：**
- `start_broadcast` - 请求开始广播
- `stop_broadcast` - 请求停止广播
- `heartbeat` - 心跳包
- `[Binary]` - 音频数据

**服务端 → 客户端：**
- `ready` - 连接就绪
- `broadcasting` - 广播已批准
- `idle` - 广播已结束
- `pong` - 心跳响应
- `error:<msg>` - 错误消息

详细 API 文档请参考 [openapi.yaml](./docs/openapi.yaml)。

## 测试

```bash
# Rust 单元测试
cd src-tauri && cargo test

# 前端测试
pnpm test

# 代码格式检查
cargo fmt --check
cargo clippy -- -D warnings
```

## 项目结构

```
broadcast-manager/
├── src/                    # Vue 3 前端源码
│   ├── components/         # Vue 组件
│   ├── stores/             # Pinia 状态管理
│   └── api/                # API 客户端
├── src-tauri/              # Tauri 后端
│   ├── src/
│   │   ├── audio/          # 音频处理模块
│   │   ├── server/         # WebSocket/HTTP 服务器
│   │   ├── config.rs       # 配置管理
│   │   └── lib.rs          # 主入口
│   └── Cargo.toml          # Rust 依赖
└── docs/                   # 文档
```

## 安全特性

- ✅ 配置文件权限限制 (0600/0700)
- ✅ 输入验证和参数检查
- ✅ DoS 防护（消息速率限制、数据包大小限制）
- ✅ 心跳机制（5秒间隔，10秒超时）
- ✅ 日志轮转（自动清理7天前日志）
- ✅ IP 白名单支持

## 贡献

欢迎贡献代码、报告 Bug 或提出新功能建议！

请参考 [开发指南](./docs/DEVELOPMENT.md) 了解如何参与开发。

## 许可证

MIT License

---

**推荐 IDE 设置**

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
