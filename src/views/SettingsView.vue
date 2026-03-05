<template>
  <div class="settings-view">
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <el-icon><Connection /></el-icon>
          <span>服务器配置</span>
        </div>
      </template>

      <el-form :model="serverConfig" label-width="140px" style="max-width: 500px">
        <el-form-item label="监听地址">
          <el-select v-model="serverConfig.bind_address" style="width: 220px">
            <el-option label="所有接口 IPv4 (0.0.0.0)" value="0.0.0.0" />
            <el-option label="所有接口 IPv6 (::)" value="::" />
            <el-option label="本地 IPv4 (127.0.0.1)" value="127.0.0.1" />
            <el-option label="本地 IPv6 (::1)" value="::1" />
            <el-option label="本地 (localhost)" value="localhost" />
          </el-select>
          <span class="form-tip">服务器监听的网络接口</span>
        </el-form-item>

        <el-form-item label="服务端口">
          <el-input-number
            v-model="serverConfig.port"
            :min="1024"
            :max="65535"
            :step="1"
          />
          <span class="form-tip">WebSocket 和 HTTP API 共同使用此端口</span>
        </el-form-item>

        <el-form-item label="最大连接数">
          <el-input-number
            v-model="serverConfig.max_connections"
            :min="1"
            :max="100"
            :step="1"
          />
          <span class="form-tip">同时连接的客户端数量上限</span>
        </el-form-item>

        <el-divider />

        <el-form-item label="启用白名单">
          <el-switch
            v-model="serverConfig.whitelist_enabled"
            active-text="已启用"
            inactive-text="已禁用"
            style="--el-switch-on-color: #13ce66; --el-switch-off-color: #dcdfe6"
          />
          <span class="form-tip">只允许白名单中的客户端发起广播请求</span>
        </el-form-item>

        <el-form-item v-if="serverConfig.whitelist_enabled" label="白名单地址">
          <div class="whitelist-container">
            <div class="whitelist-input">
              <el-input
                v-model="newWhitelistAddress"
                placeholder="输入 IP 地址（如：192.168.1.100）"
                @keyup.enter="addWhitelistAddress"
              />
              <el-button
                type="primary"
                @click="addWhitelistAddress"
                :disabled="!newWhitelistAddress.trim()"
              >
                添加
              </el-button>
            </div>
            <div class="whitelist-list">
              <el-tag
                v-for="(addr, index) in serverConfig.whitelist_addresses"
                :key="index"
                closable
                @close="removeWhitelistAddress(index)"
                class="whitelist-tag"
              >
                {{ addr }}
              </el-tag>
              <el-empty
                v-if="serverConfig.whitelist_addresses.length === 0"
                description="暂无白名单地址"
                :image-size="60"
              />
            </div>
            <div class="whitelist-hint">
              <el-icon><InfoFilled /></el-icon>
              <span>环回地址（127.0.0.1, ::1, localhost）总是被允许，无需添加</span>
            </div>
          </div>
        </el-form-item>

        <el-form-item>
          <el-button type="primary" @click="saveConfig" :loading="saving">
            保存配置
          </el-button>
          <el-button @click="loadConfig">重新加载</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 开机自启动 -->
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <el-icon><Setting /></el-icon>
          <span>应用设置</span>
        </div>
      </template>

      <el-form label-width="140px" style="max-width: 500px">
        <el-form-item label="开机自启动">
          <el-switch
            v-model="autostartEnabled"
            :loading="checkingAutostart || togglingAutostart"
            :before-change="toggleAutostart"
            active-text="已启用"
            inactive-text="已禁用"
            style="--el-switch-on-color: #13ce66; --el-switch-off-color: #dcdfe6"
          />
          <span class="form-tip">应用启动时自动运行</span>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card class="settings-card danger-zone">
      <template #header>
        <div class="card-header danger">
          <el-icon><Warning /></el-icon>
          <span>危险区域</span>
        </div>
      </template>

      <div class="danger-content">
        <p>重置所有配置将恢复为默认值，此操作不可撤销。</p>
        <el-button type="danger" @click="showResetDialog = true">
          重置所有配置
        </el-button>
      </div>
    </el-card>

    <!-- 重置确认对话框 -->
    <el-dialog
      v-model="showResetDialog"
      title="确认重置"
      width="400px"
    >
      <p>确定要将所有配置重置为默认值吗？</p>
      <p class="warning-text">此操作不可撤销，重置后需要重启应用才能生效。</p>
      <template #footer>
        <el-button @click="showResetDialog = false">取消</el-button>
        <el-button type="danger" @click="resetConfig" :loading="resetting">
          确认重置
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Connection, Warning, Setting, InfoFilled } from '@element-plus/icons-vue'
import { tauriClient, type ServerConfig } from '../composables/useTauriCommands'

// 服务器配置
const serverConfig = reactive<ServerConfig>({
  bind_address: '0.0.0.0',
  port: 8081,
  max_connections: 10,
  whitelist_enabled: false,
  whitelist_addresses: [],
})

// 白名单输入
const newWhitelistAddress = ref('')

// 状态
const saving = ref(false)
const resetting = ref(false)
const showResetDialog = ref(false)

// 开机自启动状态
const autostartEnabled = ref(false)
const checkingAutostart = ref(false)
const togglingAutostart = ref(false)

// 加载配置
async function loadConfig() {
  try {
    const config = await tauriClient.getServerConfig()
    Object.assign(serverConfig, config)
    ElMessage.success('配置加载成功')
  } catch (e) {
    ElMessage.error('加载配置失败: ' + (e as Error).message)
  }
}

// 保存配置
async function saveConfig() {
  saving.value = true
  try {
    await tauriClient.updateServerConfig(serverConfig)
    ElMessage.success('配置保存成功，重启应用后生效')
  } catch (e) {
    ElMessage.error('保存配置失败: ' + (e as Error).message)
  } finally {
    saving.value = false
  }
}

// 重置配置
async function resetConfig() {
  resetting.value = true
  try {
    await tauriClient.resetAllConfig()
    showResetDialog.value = false
    ElMessage.success('配置已重置为默认值，请重启应用')
    // 重新加载配置
    await loadConfig()
  } catch (e) {
    ElMessage.error('重置配置失败: ' + (e as Error).message)
  } finally {
    resetting.value = false
  }
}

// 检查开机自启动状态
async function checkAutostartStatus() {
  checkingAutostart.value = true
  try {
    autostartEnabled.value = await tauriClient.isAutostartEnabled()
  } catch (e) {
    console.error('检查开机自启动状态失败:', e)
    // 失败时默认为禁用
    autostartEnabled.value = false
  } finally {
    checkingAutostart.value = false
  }
}

// 切换开机自启动
async function toggleAutostart() {
  togglingAutostart.value = true
  try {
    const newState = !autostartEnabled.value
    if (newState) {
      await tauriClient.enableAutostart()
      ElMessage.success('开机自启动已启用')
    } else {
      await tauriClient.disableAutostart()
      ElMessage.success('开机自启动已禁用')
    }
    autostartEnabled.value = newState
  } catch (e) {
    ElMessage.error('操作失败: ' + (e as Error).message)
    // 刷新状态以恢复实际值
    await checkAutostartStatus()
    return false // 阻止开关切换
  } finally {
    togglingAutostart.value = false
  }
  return true // 允许开关切换
}

// 白名单管理
function addWhitelistAddress() {
  const addr = newWhitelistAddress.value.trim()
  if (!addr) {
    return
  }

  // 验证 IP 地址格式
  if (addr !== '0.0.0.0' &&
      addr !== 'localhost' &&
      !isValidIpAddress(addr)) {
    ElMessage.error('请输入有效的 IP 地址')
    return
  }

  // 检查是否已存在
  if (serverConfig.whitelist_addresses.includes(addr)) {
    ElMessage.warning('该地址已在白名单中')
    return
  }

  serverConfig.whitelist_addresses.push(addr)
  newWhitelistAddress.value = ''
  ElMessage.success('已添加白名单地址')
}

function removeWhitelistAddress(index: number) {
  serverConfig.whitelist_addresses.splice(index, 1)
  ElMessage.success('已移除白名单地址')
}

function isValidIpAddress(addr: string): boolean {
  // 简单的 IP 地址验证
  const ipv4Regex = /^(\d{1,3}\.){3}\d{1,3}$/
  const ipv6Regex = /^([0-9a-fA-F]{0,4}:){7}[0-9a-fA-F]{0,4}$/

  if (ipv4Regex.test(addr)) {
    // 检查每个部分是否在 0-255 范围内
    const parts = addr.split('.')
    return parts.every(part => {
      const num = parseInt(part, 10)
      return num >= 0 && num <= 255
    })
  }

  if (ipv6Regex.test(addr)) {
    return true
  }

  // 支持 IP 段格式（如 192.168.1.）
  if (addr.endsWith('.') && addr.split('.').length === 4) {
    const parts = addr.slice(0, -1).split('.')
    return parts.every(part => {
      const num = parseInt(part, 10)
      return !isNaN(num) && num >= 0 && num <= 255
    })
  }

  return false
}

// 初始化
onMounted(() => {
  loadConfig()
  checkAutostartStatus()
})
</script>

<style scoped>
.settings-view {
  padding: 20px;
}

.settings-card {
  margin-bottom: 20px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.card-header.danger {
  color: #f56c6c;
}

.form-tip {
  margin-left: 10px;
  font-size: 12px;
  color: #909399;
}

.danger-zone {
  border-color: #f56c6c;
}

.danger-zone :deep(.el-card__header) {
  background-color: #fef0f0;
  border-bottom-color: #f56c6c;
}

.danger-content {
  padding: 10px 0;
}

.danger-content p {
  margin: 0 0 15px;
  color: #606266;
}

.warning-text {
  color: #f56c6c;
  margin-top: 10px;
}

.whitelist-container {
  width: 100%;
}

.whitelist-input {
  display: flex;
  gap: 10px;
  margin-bottom: 15px;
}

.whitelist-input .el-input {
  flex: 1;
}

.whitelist-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  min-height: 40px;
  padding: 10px;
  background: #f5f7fa;
  border-radius: 4px;
  margin-bottom: 10px;
}

.whitelist-tag {
  font-family: 'Consolas', 'Monaco', monospace;
}

.whitelist-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  background: #e7f3ff;
  border-left: 3px solid #409eff;
  border-radius: 4px;
  font-size: 12px;
  color: #606266;
}

.whitelist-hint .el-icon {
  color: #409eff;
}
</style>
