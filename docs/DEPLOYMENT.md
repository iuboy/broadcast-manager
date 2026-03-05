# Broadcast Service 部署指南

本文档介绍如何在不同环境中部署 broadcast-manager 广播服务。

## 目录

- [系统要求](#系统要求)
- [安装方式](#安装方式)
- [配置](#配置)
- [运行](#运行)
- [故障排查](#故障排查)

---

## 系统要求

### Linux (Ubuntu/Debian)

```bash
# 系统库依赖
sudo apt-get update
sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    libopus-dev \
    libasound2-dev
```

### macOS

```bash
# 使用 Homebrew 安装依赖
brew install opus
```

### Windows

- Windows 10 或更高版本
- 无额外依赖要求

---

## 安装方式

### 方式 1: 从发布版本安装

1. 从 [Releases](../../releases) 页面下载对应平台的安装包：
   - Linux: `.deb` 或 `.AppImage`
   - macOS: `.dmg` 或 `.app`
   - Windows: `.exe` 或 `.msi`

2. 运行安装程序并按照提示完成安装

### 方式 2: 从源码构建

```bash
# 克隆仓库
git clone https://github.com/your-username/broadcast-service.git
cd broadcast-service/broadcast-manager

# 安装 pnpm (如果尚未安装)
npm install -g pnpm

# 安装依赖
pnpm install

# 构建应用
pnpm build

# 运行应用
pnpm tauri dev
```

---

## 配置

配置文件位置：
- **配置文件**: `~/.broadcast-service/broadcast-manager-config.toml`
- **环境变量**: `.env` (仅开发环境)

> **注意**: 生产环境使用 TOML 配置文件，开发环境可以使用环境变量覆盖部分配置。

### 默认配置示例

```toml
[server]
# 监听地址
bind_address = "0.0.0.0"
# 服务端口
port = 8081
# 最大连接数
max_connections = 10

[server.cors]
# 允许的跨域来源（桌面应用可使用通配符）
allowed_origins = ["*"]

[server.auth]
# 是否启用本地访问限制
enabled = true
# 允许的本地地址
local_addresses = ["127.0.0.1", "::1", "localhost"]

[server.client_whitelist]
# 是否启用客户端白名单
enabled = false
# 允许发起广播的客户端 IP
allowed_addresses = []

[server.log_level]
# 日志级别: trace, debug, info, warn, error
log_level = "info"

[ducking]
# 是否启用闪避
enabled = true
# 闪避音量 (0.0 - 1.0)
volume = 0.3
# 淡入时间（毫秒）
fade_in_ms = 400
# 淡出时间（毫秒）
fade_out_ms = 600

[music]
# 音乐目录
music_dir = "./music"
# 音乐音量
volume = 0.7

[broadcast]
# 采样率
sample_rate = 44100
# 声道数 (1=单声道, 2=立体声)
channels = 1
# 编解码格式
codec = "pcm"

[runtime]
# 主音量
master_volume = 1.0
# 音乐音量
music_volume = 1.0
# 广播音量
broadcast_volume = 1.0
# 是否启用闪避
ducking_enabled = true
# 闪避音量
ducking_volume = 0.3
# 闪避淡入时间
ducking_fade_in_ms = 500
# 闪避淡出时间
ducking_fade_out_ms = 1000

# 播放列表
playlist = []
```
# 播放列表
playlist = []
```

### 环境变量配置 (开发环境)

开发环境可以使用环境变量来覆盖配置。创建 `.env` 文件：

```bash
# 复制环境变量模板
cp .env.example .env

# 编辑 .env 文件根据需要修改配置
```

常用环境变量：

```bash
# 日志级别
RUST_LOG=info

# 服务端口
BROADCAST_PORT=8081

# 其他配置请参考 .env.example 文件
```

> **注意**: 环境变量仅用于开发调试，生产环境请使用 TOML 配置文件。

### 配置说明

#### 服务器配置

- **bind_address**: 监听地址
  - `0.0.0.0` - 接受所有网络接口的连接
  - `127.0.0.1` - 仅接受本地连接
  
- **port**: 服务端口（默认 8081）

- **max_connections**: 最大并发连接数

- **log_level**: 日志级别
  - `trace` - 最详细，仅用于调试
  - `debug` - 调试信息
  - `info` - 一般信息（生产环境推荐）
  - `warn` - 警告信息
  - `error` - 仅错误信息

#### 白名单配置

客户端白名单用于控制哪些客户端可以发起广播请求：

```toml
[server.client_whitelist]
enabled = true
allowed_addresses = ["192.168.1.100", "192.168.1.101"]
```

> 注意：环回地址（127.0.0.1, ::1, localhost）总是被允许。

---

## 运行

### 开发模式

```bash
pnpm tauri dev
```

### 生产模式

从安装包安装后，直接启动应用程序即可。

### 作为系统服务运行 (Linux)

创建 systemd 服务文件 `/etc/systemd/system/broadcast-manager.service`：

```ini
[Unit]
Description=Broadcast Manager Service
After=network.target

[Service]
Type=simple
User=your-username
WorkingDirectory=/home/your-username
ExecStart=/home/your-username/.local/bin/broadcast-manager
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

启动服务：

```bash
sudo systemctl daemon-reload
sudo systemctl enable broadcast-manager
sudo systemctl start broadcast-manager
```

---

## 故障排查

### 端口被占用

**问题**: 应用启动失败，提示端口已被占用

**解决方案**:
1. 检查端口占用: `lsof -i :8081` (Linux/macOS) 或 `netstat -ano | findstr :8081` (Windows)
2. 修改配置文件中的端口号
3. 或停止占用端口的程序

### 无法连接 WebSocket

**问题**: 客户端无法连接到服务

**解决方案**:
1. 检查防火墙设置
2. 确认服务正在运行
3. 检查 IP 白名单配置
4. 查看日志文件: `~/.broadcast-service/logs/broadcast-manager.YYYY-MM-DD`

### 音频输出无声音

**问题**: 广播开始但无声音输出

**解决方案**:
1. 检查系统音量设置
2. 确认 `master_volume` 和 `broadcast_volume` 配置正确
3. 检查音频设备是否正常工作

### 配置文件权限问题

**问题**: 配置无法保存

**解决方案**:
```bash
# 检查配置目录权限
ls -la ~/.broadcast-service/

# 修复权限
chmod 700 ~/.broadcast-service
chmod 600 ~/.broadcast-service/broadcast-manager-config.toml
```

---

## 日志

日志文件位置：`~/.broadcast-service/logs/`

日志自动按天轮转，默认保留最近 7 天的日志。

查看日志：

```bash
# 查看今天的日志
tail -f ~/.broadcast-service/logs/broadcast-manager.$(date +%Y-%m-%d)

# 查看错误日志
grep ERROR ~/.broadcast-service/logs/broadcast-manager.*
```

---

## 安全建议

1. **生产环境配置**
   - 设置 `log_level = "warn"` 或 `"error"`
   - 启用客户端白名单 `client_whitelist.enabled = true`
   - 如需远程访问，配置防火墙规则

2. **网络安全**
   - 不要在公网直接暴露服务端口
   - 使用 VPN 或 SSH 隧道进行远程访问
   - 定期更新依赖

3. **备份配置**
   ```bash
   cp ~/.broadcast-service/broadcast-manager-config.toml ~/.broadcast-service/backup-config.toml
   ```

---

## 更新

### 从发布版本更新

1. 下载新版本的安装包
2. 卸载旧版本（配置文件会保留）
3. 安装新版本

### 从源码更新

```bash
cd broadcast-service/broadcast-manager
git pull origin main
pnpm install
pnpm build
```
