# mihomo_sdk

用于管理 [mihomo](https://github.com/MetaCubeX/mihomo) 进程的 Rust SDK。提供完整的进程生命周期管理（启动 / 停止 / 重启）以及通过 Windows Named Pipe 通信的全类型异步 API 客户端。

## 功能特性

- **进程管理** — 启动、停止、重启 mihomo 二进制文件，支持完整命令行参数（`-f`、`-d`、`-ext-ctl-pipe`、`-secret` 等）
- **API 就绪检测** — `wait_ready()` 轮询等待 mihomo API 可用后再发起调用
- **全部 REST API 封装** — 覆盖 mihomo 官方文档的所有 HTTP 端点，返回强类型结构体
- **Named Pipe 传输层** — 通过 `\\.\pipe\mihomo` 直接通信，无需额外网络端口
- **异步优先** — 基于 `tokio` 运行时，所有操作均为 `async fn`
- **线程安全** — `MihomoManager` 内部使用 `Arc<Mutex<>>` 保护，可安全 `Clone` 后跨线程共享

## 依赖

```toml
[dependencies]
mihomo_sdk = { path = "../mihomo_sdk" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"  # 如果需要构造 patch_configs 等 JSON 值
```

## 快速开始

```rust
use mihomo_sdk::MihomoManager;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建管理器
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");

    // 2. 启动进程并等待 API 就绪
    let pid = mgr.start().await?;
    println!("mihomo 已启动, PID={pid}");
    mgr.wait_ready(20, Duration::from_millis(500)).await?;

    // 3. 调用 API
    let ver = mgr.get_version().await?;
    println!("版本: {}", ver.version);

    let proxies = mgr.get_proxies().await?;
    println!("代理数量: {}", proxies.proxies.len());

    // 4. 停止进程
    mgr.stop().await?;
    Ok(())
}
```

## 项目结构

```
mihomo_sdk/src/
├── lib.rs                # MihomoManager — 进程管理 + 入口
└── api/
    ├── mod.rs            # 模块声明与 re-export
    ├── transport.rs      # PipeTransport — Named Pipe HTTP 传输层
    ├── models.rs         # 所有 API 请求/响应的数据结构
    └── mihomo.rs         # 为 MihomoManager 实现的全部 API 方法
```

### 各层职责

| 层 | 文件 | 说明 |
|---|---|---|
| 入口层 | `lib.rs` | `MihomoManager` 结构体：持有进程句柄和 `PipeTransport`，提供进程管理方法 |
| 业务层 | `api/mihomo.rs` | 为 `MihomoManager` 实现所有 mihomo REST API 方法 |
| 数据层 | `api/models.rs` | 所有请求体/响应体的强类型定义，均实现 `Serialize + Deserialize` |
| 传输层 | `api/transport.rs` | 底层 HTTP/1.1 报文构建、Named Pipe 连接、响应解析 |

## 进程管理

### 创建管理器

```rust
use mihomo_sdk::{MihomoManager, PipeTransport};

// 默认配置（pipe 地址 \\.\pipe\mihomo）
let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");

// 自定义 pipe 地址和超时
let transport = PipeTransport::new()
    .with_pipe_name(r"\\.\pipe\my_mihomo")
    .with_timeout(std::time::Duration::from_secs(30));
let mgr = MihomoManager::with_transport("./mihomo.exe", "./config.yaml", transport);
```

### 配置启动参数

所有配置方法对应 mihomo `main.go` 中的命令行参数：

```rust
// -d <home_dir> — 设置工作目录
mgr.set_home_dir("/opt/mihomo").await;

// -ext-ctl-pipe <addr> — 覆盖 Named Pipe 地址
mgr.set_ext_ctl_pipe(r"\\.\pipe\my_mihomo").await;

// -secret <secret> — 覆盖 API 密钥
// 注意: Named Pipe 通道服务端不校验 secret，此参数主要影响 TCP/TLS 通道
mgr.set_secret("my_secret").await;

// 额外参数（如 -m 启用 geodata 模式）
mgr.add_extra_arg("-m").await;

// Drop 时是否自动 kill 子进程（默认 true）
mgr.set_kill_on_drop(false).await;
```

最终构建的命令行形如：

```
mihomo.exe -f config.yaml [-d /home/dir] [-ext-ctl-pipe \\.\pipe\mihomo] [-secret xxx] [额外参数...]
```

### 生命周期控制

```rust
// 启动
let pid = mgr.start().await?;

// 等待 API 就绪（最多重试 20 次，每次间隔 500ms）
mgr.wait_ready(20, Duration::from_millis(500)).await?;

// 或一步到位
let pid = mgr.start_and_wait(20, Duration::from_millis(500)).await?;

// 查询状态
match mgr.status().await {
    ProcessStatus::Running(pid) => println!("运行中, PID={pid}"),
    ProcessStatus::Stopped => println!("已停止"),
}

// 快捷判断
if mgr.is_running().await { /* ... */ }

// 停止
mgr.stop().await?;

// 重启（先停后启，若未运行则直接启动）
let pid = mgr.restart().await?;
```

### 路径更新

运行时可更新路径，下次 `start()` / `restart()` 生效：

```rust
mgr.set_binary_path("./mihomo-v2.exe").await;
mgr.set_config_path("./new-config.yaml").await;
```

### 错误类型

```rust
pub enum ProcessError {
    BinaryNotFound(PathBuf),   // 可执行文件不存在
    ConfigNotFound(PathBuf),   // 配置文件不存在
    AlreadyRunning(u32),       // 进程已在运行（附 PID）
    NotRunning,                // 进程未运行
    Io(std::io::Error),        // I/O 错误（含 pipe 连接失败、超时等）
    NotReady(u32),             // API 未就绪（附重试次数）
}
```

## API 参考

所有 API 方法直接在 `MihomoManager` 上调用，内部通过 `self.api()` 获取 `PipeTransport` 发送请求。

也可通过 `mgr.api()` 获取底层传输层引用，直接发送原始请求：

```rust
let resp = mgr.api().get("/version").await?;
println!("原始响应: {}", resp.body);
```

### 健康检查 / 版本

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `hello()` | `GET /` | `HelloResponse` | 健康检查，返回 `{"hello":"mihomo"}` |
| `get_version()` | `GET /version` | `VersionResponse` | 获取版本号和 meta 标志 |

```rust
let hello = mgr.hello().await?;
assert_eq!(hello.hello, "mihomo");

let ver = mgr.get_version().await?;
println!("mihomo {} (meta={})", ver.version, ver.meta);
```

### 运行配置

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `get_configs()` | `GET /configs` | `ConfigResponse` | 获取当前运行配置 |
| `reload_configs(path, payload)` | `PUT /configs?force=true` | `()` | 强制重新加载配置 |
| `reload_configs_no_force(path, payload)` | `PUT /configs` | `()` | 非强制重新加载配置 |
| `patch_configs(json_value)` | `PATCH /configs` | `()` | 更新部分配置字段 |
| `update_geo_database()` | `POST /configs/geo` | `()` | 更新 GEO 数据库 |

```rust
// 获取配置
let cfg = mgr.get_configs().await?;
println!("模式: {}, 端口: {}", cfg.mode, cfg.mixed_port);

// 热更新部分配置
mgr.patch_configs(serde_json::json!({
    "mixed-port": 7890,
    "allow-lan": true,
    "mode": "rule"
})).await?;

// 重新加载配置文件
mgr.reload_configs("", "").await?;                     // 使用当前配置路径
mgr.reload_configs("/abs/path/to/config.yaml", "").await?; // 指定绝对路径

// 更新 GEO 数据库
mgr.update_geo_database().await?;
```

> **注意**：`reload_configs` 的 `path` 参数如果不为空，必须是绝对路径且在 `SAFE_PATHS` 环境变量中。

### 代理

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `get_proxies()` | `GET /proxies` | `ProxiesResponse` | 获取所有代理信息 |
| `get_proxy(name)` | `GET /proxies/:name` | `ProxyInfo` | 获取指定代理信息 |
| `select_proxy(group, proxy)` | `PUT /proxies/:name` | `()` | 在 Selector 组中选择代理 |
| `test_proxy_delay(name, url, timeout_ms)` | `GET /proxies/:name/delay` | `DelayResponse` | 测试代理延迟 |
| `test_proxy_delay_with_expected(...)` | `GET /proxies/:name/delay` | `DelayResponse` | 测试延迟（附期望状态码） |
| `unfixed_proxy(name)` | `DELETE /proxies/:name` | `()` | 清除自动策略组的 fixed 选择 |

```rust
// 列出所有代理
let proxies = mgr.get_proxies().await?;
for (name, info) in &proxies.proxies {
    println!("{}: {} (type={})", name, info.name, info.proxy_type);
}

// 选择节点
mgr.select_proxy("Proxy", "Japan").await?;

// 测试延迟
let delay = mgr.test_proxy_delay(
    "Japan",
    "https://www.gstatic.com/generate_204",
    5000,
).await?;
println!("延迟: {}ms", delay.delay);

// 清除 URLTest/Fallback 类型组的 fixed 选择
mgr.unfixed_proxy("Auto").await?;
```

### 策略组

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `get_groups()` | `GET /group` | `GroupsResponse` | 获取所有策略组 |
| `get_group(name)` | `GET /group/:name` | `GroupInfo` | 获取指定策略组 |
| `test_group_delay(name, url, timeout_ms)` | `GET /group/:name/delay` | `GroupDelayResponse` | 批量测试组内节点延迟 |
| `test_group_delay_with_expected(...)` | `GET /group/:name/delay` | `GroupDelayResponse` | 批量测试延迟（附期望状态码） |

```rust
let groups = mgr.get_groups().await?;
for g in &groups.proxies {
    println!("策略组: {} (type={}, now={})", g.name, g.group_type, g.now);
}

// 批量测试延迟，返回 HashMap<节点名, 延迟ms>
let delays = mgr.test_group_delay(
    "Proxy",
    "https://www.gstatic.com/generate_204",
    5000,
).await?;
for (name, delay) in &delays {
    println!("  {}: {}ms", name, delay);
}
```

> **注意**：`GroupsResponse.proxies` 是 `Vec<GroupInfo>`（数组），不是 Map。这与 `/proxies` 端点不同。

### 代理集合（Proxy Providers）

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `get_proxy_providers()` | `GET /providers/proxies` | `ProxyProvidersResponse` | 获取所有代理集合 |
| `get_proxy_provider(name)` | `GET /providers/proxies/:name` | `ProxyProviderInfo` | 获取指定代理集合 |
| `update_proxy_provider(name)` | `PUT /providers/proxies/:name` | `()` | 触发更新代理集合 |
| `healthcheck_proxy_provider(name)` | `GET /providers/proxies/:name/healthcheck` | `()` | 触发健康检查 |
| `get_proxy_in_provider(provider, proxy)` | `GET /providers/proxies/:p/:n` | `ProxyInfo` | 获取集合内指定代理 |
| `healthcheck_proxy_in_provider(...)` | `GET /providers/proxies/:p/:n/healthcheck` | `DelayResponse` | 测试集合内指定代理延迟 |

```rust
let providers = mgr.get_proxy_providers().await?;
for (name, info) in &providers.providers {
    println!("Provider: {} ({}, {} 个代理)", name, info.vehicle_type, info.proxies.len());
}

// 更新订阅
mgr.update_proxy_provider("my-subscription").await?;

// 健康检查
mgr.healthcheck_proxy_provider("my-subscription").await?;
```

### 规则

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `get_rules()` | `GET /rules` | `RulesResponse` | 获取所有规则 |
| `disable_rules(rules)` | `PATCH /rules/disable` | `()` | 禁用/启用指定规则（临时，重启失效） |

```rust
let rules = mgr.get_rules().await?;
for rule in &rules.rules {
    println!("[{}] {} {} -> {}", rule.index, rule.rule_type, rule.payload, rule.proxy);
    if let Some(extra) = &rule.extra {
        println!("    命中: {} 次, 禁用: {}", extra.hit_count, extra.disabled);
    }
}

// 禁用第 0 条规则，启用第 1 条
use std::collections::HashMap;
let mut req: HashMap<i64, bool> = HashMap::new();
req.insert(0, true);   // 禁用
req.insert(1, false);  // 启用
mgr.disable_rules(&req).await?;
```

### 规则集合（Rule Providers）

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `get_rule_providers()` | `GET /providers/rules` | `RuleProvidersResponse` | 获取所有规则集合 |
| `update_rule_provider(name)` | `PUT /providers/rules/:name` | `()` | 更新指定规则集合 |

```rust
let rule_providers = mgr.get_rule_providers().await?;
for (name, info) in &rule_providers.providers {
    println!("{}: {} 条规则 ({})", name, info.rule_count, info.behavior);
}

mgr.update_rule_provider("my-rules").await?;
```

### 连接

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `get_connections()` | `GET /connections` | `ConnectionsResponse` | 获取当前连接快照 |
| `close_all_connections()` | `DELETE /connections` | `()` | 关闭所有连接 |
| `close_connection(id)` | `DELETE /connections/:id` | `()` | 关闭指定连接 |

```rust
let conns = mgr.get_connections().await?;
println!("总上传: {}, 总下载: {}", conns.upload_total, conns.download_total);

if let Some(list) = &conns.connections {
    for c in list {
        println!("  {} -> {}:{} ({})",
            c.metadata.source_ip,
            c.metadata.host,
            c.metadata.destination_port,
            c.rule,
        );
    }
}

// 关闭所有连接
mgr.close_all_connections().await?;

// 关闭指定连接
mgr.close_connection("abc-123").await?;
```

### DNS

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `dns_query(name, query_type)` | `GET /dns/query` | `DnsQueryResponse` | DNS 查询 |

```rust
let result = mgr.dns_query("example.com", "A").await?;
println!("状态码: {}", result.status);
if let Some(answers) = &result.answer {
    for a in answers {
        println!("  {} TTL={} -> {}", a.name, a.ttl, a.data);
    }
}

// query_type 为空字符串时默认使用 "A"
let result = mgr.dns_query("example.com", "").await?;
```

### 缓存

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `flush_fakeip_cache()` | `POST /cache/fakeip/flush` | `()` | 清除 FakeIP 缓存 |
| `flush_dns_cache()` | `POST /cache/dns/flush` | `()` | 清除 DNS 缓存 |

```rust
mgr.flush_fakeip_cache().await?;
mgr.flush_dns_cache().await?;
```

### 重启 / 升级

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `restart_core()` | `POST /restart` | `StatusResponse` | 重启 mihomo 内核进程 |
| `upgrade_core(channel, force)` | `POST /upgrade` | `StatusResponse` | 更新内核二进制 |
| `upgrade_ui()` | `POST /upgrade/ui` | `StatusResponse` | 更新外部 UI 面板 |
| `upgrade_geo()` | `POST /upgrade/geo` | `()` | 更新 GEO 数据库 |

```rust
// 重启内核
// 注意: 调用后 mihomo 进程会 exec 重启自身，pipe 连接会断开
let status = mgr.restart_core().await?;
println!("{}", status.status); // "ok"

// 更新内核
let status = mgr.upgrade_core(None, false).await?;          // 默认通道
let status = mgr.upgrade_core(Some("alpha"), true).await?;  // 指定通道 + 强制

// 更新 UI（需要配置文件中设置了 external-ui）
mgr.upgrade_ui().await?;

// 更新 GEO 数据库
mgr.upgrade_geo().await?;
```

> **注意**：`restart_core()` 会导致 mihomo 进程重新执行自身。在 Windows 上会启动新进程后 `os.Exit(0)`。调用后当前 pipe 连接会断开，需要重新等待 API 就绪。

### 调试

| 方法 | 端点 | 返回类型 | 说明 |
|---|---|---|---|
| `debug_gc()` | `PUT /debug/gc` | `()` | 手动触发 Go runtime GC |

```rust
// 需要 mihomo 以 log-level: debug 启动
mgr.debug_gc().await?;
```

## 数据模型

所有数据结构定义在 `api/models.rs` 中，均实现了 `Debug`、`Clone`、`Serialize`、`Deserialize`。

### 通用

| 结构体 | 说明 |
|---|---|
| `HelloResponse` | `GET /` 响应 — `{ hello: String }` |
| `VersionResponse` | 版本信息 — `{ version: String, meta: bool }` |
| `StatusResponse` | 操作状态 — `{ status: String }`（值通常为 `"ok"`） |
| `ApiError` | 错误响应 — `{ message: String }` |
| `ConfigPatchRequest` | `PATCH /configs` 请求体 — 包装 `serde_json::Value` |
| `ConfigReloadRequest` | `PUT /configs` 请求体 — `{ path, payload }` |
| `UpgradeRequest` | 升级请求体 — `{ path, payload }` |

### 流式数据（Streaming）

以下结构体用于 `GET /traffic`、`GET /memory`、`GET /logs` 等流式端点。这些端点返回持续的 JSON 行流，当前 SDK 未封装流式读取，但数据结构已定义，可配合自定义流式读取逻辑使用。

| 结构体 | 说明 |
|---|---|
| `TrafficEntry` | 流量 — `{ up, down, upTotal, downTotal }` (i64) |
| `MemoryEntry` | 内存 — `{ inuse, oslimit }` (u64, 单位 bytes) |
| `LogEntry` | 日志（默认格式） — `{ type, payload }` |
| `LogStructured` | 日志（structured 格式） — `{ time, level, message, fields }` |
| `LogStructuredField` | 结构化日志字段 — `{ key, value }` |

### 配置

| 结构体 | 说明 |
|---|---|
| `ConfigResponse` | 运行配置快照，包含 `port`、`mixed-port`、`mode`、`tun` 等常用字段，未知字段收入 `extra: HashMap` |
| `TunConfig` | TUN 配置 — `enable`、`device`、`stack`、`auto-route` 等 |

### 代理

| 结构体 | 说明 |
|---|---|
| `ProxiesResponse` | `{ proxies: HashMap<String, ProxyInfo> }` |
| `ProxyInfo` | 代理详情 — `name`、`type`、`udp`、`history`、`all`、`now` 等，未知字段收入 `extra` |
| `SelectProxyRequest` | 选择代理请求体 — `{ name }` |
| `ProxyDelayEntry` | 延迟历史条目 — `{ time, delay }` |
| `DelayResponse` | 延迟测试结果 — `{ delay }` (u64, ms) |

### 策略组

| 结构体 | 说明 |
|---|---|
| `GroupsResponse` | `{ proxies: Vec<GroupInfo> }` — **注意是数组** |
| `GroupInfo` | 策略组详情 — `name`、`type`、`now`、`all`、`history` 等 |
| `GroupDelayResponse` | 类型别名 `HashMap<String, u64>` — 节点名 → 延迟 ms |

### 代理集合

| 结构体 | 说明 |
|---|---|
| `ProxyProvidersResponse` | `{ providers: HashMap<String, ProxyProviderInfo> }` |
| `ProxyProviderInfo` | 代理集合详情 — `name`、`vehicleType`、`proxies`、`updatedAt`、`subscriptionInfo` |
| `SubscriptionInfo` | 订阅信息 — `Upload`、`Download`、`Total`、`Expire` |

### 规则

| 结构体 | 说明 |
|---|---|
| `RulesResponse` | `{ rules: Vec<RuleInfo> }` |
| `RuleInfo` | 规则条目 — `index`、`type`、`payload`、`proxy`、`size`、`extra` |
| `RuleExtra` | 规则元数据 — `disabled`、`hitCount`、`hitAt`、`missCount`、`missAt` |
| `DisableRulesRequest` | 类型别名 `HashMap<i64, bool>` — 规则索引 → 是否禁用 |
| `RuleProvidersResponse` | `{ providers: HashMap<String, RuleProviderInfo> }` |
| `RuleProviderInfo` | 规则集合详情 — `name`、`behavior`、`ruleCount`、`vehicleType` |

### 连接

| 结构体 | 说明 |
|---|---|
| `ConnectionsResponse` | `downloadTotal`、`uploadTotal`、`connections`（可为 `null`）、`memory` |
| `ConnectionInfo` | 连接详情 — `id`、`metadata`、`upload`、`download`、`start`、`chains`、`rule` |
| `ConnectionMetadata` | 连接元数据 — `network`、`sourceIP`、`host`、`processPath` 等 |

### DNS

| 结构体 | 说明 |
|---|---|
| `DnsQueryResponse` | DNS 查询结果 — `Status`、`Question`、`Answer`、`Authority`、`Additional` 等 |
| `DnsQuestion` | 查询段 — `Name`、`Qtype`、`Qclass` |
| `DnsAnswer` | 应答段 — `name`、`TTL`、`type`、`data` |

## 传输层

`PipeTransport` 负责通过 Windows Named Pipe 发送原始 HTTP/1.1 请求并解析响应。通常不需要直接使用，但支持以下自定义：

```rust
use mihomo_sdk::PipeTransport;
use std::time::Duration;

let transport = PipeTransport::new()
    .with_pipe_name(r"\\.\pipe\my_mihomo")  // 自定义 pipe 名称
    .with_timeout(Duration::from_secs(30))   // 请求超时
    .with_secret("my_token");                // API 密钥（pipe 通道通常不需要）

// 直接使用
let resp = transport.get("/version").await?;
let resp = transport.put("/configs?force=true", r#"{"path":"","payload":""}"#).await?;
let resp = transport.post("/cache/fakeip/flush", None).await?;
let resp = transport.patch("/configs", r#"{"mixed-port":7890}"#).await?;
let resp = transport.delete("/connections").await?;
```

### 关于 Named Pipe 认证

mihomo 源码中，Named Pipe 通道的服务端**不校验 secret**（`server.go` 中 `startPipe` 向 `router()` 传入空字符串作为 secret）。因此通过 pipe 访问 API 不需要设置 `Authorization` 头。`PipeTransport::with_secret()` 主要用于兼容性考虑。

## 注意事项

1. **仅支持 Windows** — Named Pipe 是 Windows 特有机制，`tokio::net::windows::named_pipe` 仅在 Windows 上可用
2. **mihomo 配置要求** — 需要在 mihomo 配置文件中启用 `external-controller-pipe`：
   ```yaml
   external-controller-pipe: \\.\pipe\mihomo
   ```
   或通过 `-ext-ctl-pipe` 命令行参数覆盖
3. **流式端点** — `/logs`、`/traffic`、`/memory`、`/connections`（WebSocket）等流式端点当前未封装，数据模型已定义
4. **`restart_core()` 的副作用** — 调用后 mihomo 会 exec 重启自身，当前 pipe 连接断开。如果是由本 SDK 启动的进程，`MihomoManager` 的进程句柄会失效，建议重新 `start_and_wait()`
5. **线程安全** — `MihomoManager` 实现了 `Clone`，内部通过 `Arc<Mutex<>>` 保护，可安全在多个 tokio task 间共享

## 许可证

MIT