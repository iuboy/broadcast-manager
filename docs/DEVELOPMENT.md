# Broadcast Service 开发指南

本文档介绍如何参与 broadcast-service 项目的开发。

## 目录

- [项目结构](#项目结构)
- [开发环境设置](#开发环境设置)
- [代码规范](#代码规范)
- [测试](#测试)
- [构建](#构建)
- [提交代码](#提交代码)

---

## 项目结构

```
broadcast-service/
├── broadcast-manager/           # 桌面管理应用
│   ├── src/                    # Vue 3 前端源码
│   │   ├── components/         # Vue 组件
│   │   ├── stores/             # Pinia 状态管理
│   │   ├── api/                # API 客户端
│   │   └── __tests__/          # 前端测试
│   ├── src-tauri/              # Tauri 后端
│   │   ├── src/
│   │   │   ├── audio/          # 音频处理模块
│   │   │   ├── server/         # WebSocket/HTTP 服务器
│   │   │   ├── config.rs       # 配置管理
│   │   │   └── lib.rs          # 主入口
│   │   ├── Cargo.toml          # Rust 依赖
│   │   └── capabilities/       # Tauri 权限配置
│   └── docs/                   # 文档
├── broadcast-client/           # 客户端应用
│   └── ...
└── README.md
```

### 核心模块说明

#### 音频处理 (`src-tauri/src/audio/`)

- `actor.rs` - 音频执行器（Actor 模式）
- `engine/` - 音频引擎
  - `broadcast.rs` - 广播音频处理
  - `mixer/` - 音频混音器
- `decoder/` - 音频解码器（支持 MP3、WAV、FLAC 等）
- `playlist.rs` - 播放列表管理

#### 服务器 (`src-tauri/src/server/`)

- `websocket.rs` - WebSocket 连接处理
- `http.rs` - HTTP API 端点
- `broadcast.rs` - 广播状态管理
- `auth.rs` - 客户端认证

---

## 开发环境设置

### 前置要求

- **Node.js** >= 24.0
- **pnpm** >= 10.0
- **Rust** >= 1.80
- **系统音频库**:
  - Linux: `libopus-dev`, `libasound2-dev`
  - macOS: `opus` (via Homebrew)

### 安装步骤

1. **克隆仓库**
   ```bash
   git clone https://github.com/your-username/broadcast-service.git
   cd broadcast-service
   ```

2. **安装前端依赖**
   ```bash
   cd broadcast-manager
   pnpm install
   ```

3. **安装 Tauri CLI** (如果需要)
   ```bash
   cargo install tauri-cli --version "^2.0.0"
   ```

4. **运行开发服务器**
   ```bash
   pnpm tauri dev
   ```

---

## 代码规范

### Rust 代码规范

1. **格式化**: 使用 `rustfmt`
   ```bash
   cd src-tauri
   cargo fmt
   ```

2. **Linting**: 使用 `clippy`
   ```bash
   cd src-tauri
   cargo clippy -- -D warnings
   ```

3. **命名规范**:
   - 模块/函数: `snake_case`
   - 类型/结构体: `PascalCase`
   - 常量: `SCREAMING_SNAKE_CASE`

4. **文档注释**:
   ```rust
   /// 函数简短描述
   ///
   /// 详细说明...
   ///
   /// # Examples
   /// ```
   /// let result = function();
   /// ```
   pub fn function() -> u32 {
       42
   }
   ```

### Vue/TypeScript 代码规范

1. **组件命名**: `PascalCase`
2. **组合式函数**: `use` 前缀，如 `useBroadcastState`
3. **类型定义**: 使用 TypeScript 接口

### 提交信息规范

遵循 [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

[optional body]
```

类型：
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式（不影响功能）
- `refactor`: 代码重构
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建/工具相关

示例：
```
feat(audio): 添加 Opus 编解码支持

- 实现 Opus 解码器
- 添加格式自动检测
- 更新配置选项
```

---

## 测试

### 运行所有测试

```bash
# Rust 测试
cd src-tauri
cargo test

# 前端测试
cd ..
pnpm test
```

### Rust 单元测试

测试文件通常与源码放在同一文件中，使用 `#[cfg(test)]` 模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        assert_eq!(2 + 2, 4);
    }
}
```

### 前端测试

使用 Vitest 编写测试：

```typescript
import { describe, it, expect } from 'vitest';

describe('Utility Functions', () => {
  it('should format volume correctly', () => {
    expect(formatVolume(0.5)).toBe('50');
  });
});
```

### 测试覆盖率

```bash
# 前端覆盖率
pnpm coverage
```

---

## 构建

### 开发构建

```bash
pnpm tauri dev
```

### 生产构建

```bash
pnpm build
```

构建产物位于 `src-tauri/target/release/bundle/`

### 跨平台构建

使用 GitHub Actions 自动构建：

- **Ubuntu**: 生成 `.deb` 和 `.AppImage`
- **macOS**: 生成 `.dmg` 和 `.app`
- **Windows**: 生成 `.exe` 和 `.msi`

---

## 开发工作流

### 添加新功能

1. 创建功能分支：`git checkout -b feat/your-feature`
2. 进行开发和测试
3. 提交代码并推送到远程
4. 创建 Pull Request

### Bug 修复

1. 创建修复分支：`git checkout -b fix/issue-123`
2. 复现并修复 Bug
3. 添加回归测试
4. 提交代码

### 调试技巧

#### Rust 代码调试

```bash
# 启用调试日志
RUST_LOG=debug pnpm tauri dev

# 查看日志
tail -f ~/.broadcast-service/logs/broadcast-manager.$(date +%Y-%m-%d)
```

#### 前端调试

使用浏览器开发者工具（在开发模式下按 F12）

#### WebSocket 调试

使用 `wscat` 工具测试 WebSocket 连接：

```bash
npm install -g wscat
wscat -c "ws://localhost:8081/ws?codec=pcm&sample_rate=44100&channels=1"
```

---

## API 开发

### WebSocket 消息协议

客户端 → 服务端：
- `start_broadcast` - 请求开始广播
- `stop_broadcast` - 请求停止广播
- `heartbeat` - 心跳包
- `[Binary]` - 音频数据

服务端 → 客户端：
- `ready` - 连接就绪
- `broadcasting` - 广播已批准
- `idle` - 广播已结束
- `pong` - 心跳响应
- `error:<msg>` - 错误消息

### HTTP API

- `GET /api/health` - 健康检查

详细 API 文档见 [openapi.yaml](./openapi.yaml)

---

## 性能优化建议

1. **音频处理**
   - 使用无锁数据结构 (`ringbuf`)
   - 最小化内存拷贝
   - 使用适当的缓冲区大小

2. **WebSocket**
   - 批量处理小消息
   - 使用二进制协议而非 JSON

3. **前端**
   - 使用虚拟滚动处理长列表
   - 防抖高频事件
   - 懒加载组件

---

## 常见问题

### Q: 如何添加新的音频格式支持？

A: 在 `src-tauri/src/audio/decoder/` 中添加新的解码器实现，并更新 `Decoder` 枚举。

### Q: 如何修改 WebSocket 消息协议？

A: 修改 `src-tauri/src/server/websocket.rs` 中的消息处理逻辑，并同步更新前端 API 客户端。

### Q: 如何添加新的 Tauri 命令？

A: 在 `src-tauri/src/lib.rs` 中使用 `#[tauri::command]` 宏定义命令，并在 `main()` 中注册。

---

## 资源链接

- [Tauri 文档](https://tauri.app/)
- [Vue 3 文档](https://vuejs.org/)
- [Rust 文档](https://www.rust-lang.org/)
- [Axum 文档](https://docs.rs/axum/)
- [项目 Issue 追踪](https://github.com/your-username/broadcast-service/issues)

---

## 贡献指南

我们欢迎各种形式的贡献！

1. 报告 Bug
2. 讨论代码状态
3. 提交修复
4. 提出新功能
5. 成为维护者

在提交 PR 前，请确保：
- 代码通过所有测试
- 代码符合项目规范
- 更新了相关文档
- 添加了必要的测试

感谢您的贡献！
