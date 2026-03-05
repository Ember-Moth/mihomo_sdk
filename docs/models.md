# 数据模型

本文档详细列出 `mihomo_sdk` 中所有请求/响应数据结构的字段说明、JSON 键名映射、
序列化/反序列化行为及使用注意事项。

所有结构体定义在 `src/api/models.rs` 中，通过 `mihomo_sdk::api::models::*`
或 `mihomo_sdk::api::*` 导入。

---

## 目录

- [通用](#通用)
  - [`HelloResponse`](#helloresponse)
  - [`VersionResponse`](#versionresponse)
  - [`StatusResponse`](#statusresponse)
  - [`ApiError`](#apierror)
- [流式数据（Streaming）](#流式数据streaming)
  - [`TrafficEntry`](#trafficentry)
  - [`MemoryEntry`](#memoryentry)
  - [`LogEntry`](#logentry)
  - [`LogStructured`](#logstructured)
  - [`LogStructuredField`](#logstructuredfield)
- [配置](#配置)
  - [`ConfigResponse`](#configresponse)
  - [`TunConfig`](#tunconfig)
  - [`ConfigPatchRequest`](#configpatchrequest)
  - [`ConfigReloadRequest`](#configreloadrequest)
- [代理](#代理)
  - [`ProxiesResponse`](#proxiesresponse)
  - [`ProxyInfo`](#proxyinfo)
  - [`SelectProxyRequest`](#selectproxyrequest)
  - [`ProxyDelayEntry`](#proxydelayentry)
  - [`DelayResponse`](#delayresponse)
- [策略组](#策略组)
  - [`GroupsResponse`](#groupsresponse)
  - [`GroupInfo`](#groupinfo)
  - [`GroupDelayResponse`](#groupdelayresponse)
- [代理集合（Proxy Providers）](#代理集合proxy-providers)
  - [`ProxyProvidersResponse`](#proxyprovidersresponse)
  - [`ProxyProviderInfo`](#proxyproviderinfo)
  - [`SubscriptionInfo`](#subscriptioninfo)
- [规则集合（Rule Providers）](#规则集合rule-providers)
  - [`RuleProvidersResponse`](#ruleprovidersresponse)
  - [`RuleProviderInfo`](#ruleproviderinfo)
- [规则](#规则)
  - [`RulesResponse`](#rulesresponse)
  - [`RuleInfo`](#ruleinfo)
  - [`RuleExtra`](#ruleextra)
  - [`DisableRulesRequest`](#disablerulesrequest)
- [连接](#连接)
  - [`ConnectionsResponse`](#connectionsresponse)
  - [`ConnectionInfo`](#connectioninfo)
  - [`ConnectionMetadata`](#connectionmetadata)
- [DNS](#dns)
  - [`DnsQueryResponse`](#dnsqueryresponse)
  - [`DnsQuestion`](#dnsquestion)
  - [`DnsAnswer`](#dnsanswer)
- [升级](#升级)
  - [`UpgradeRequest`](#upgraderequest)
- [传输层](#传输层)
  - [`HttpResponse`](#httpresponse)
  - [`PipeStream<T>`](#pipestreamlttgt)
- [设计说明](#设计说明)
  - [extra 字段模式](#extra-字段模式)
  - [serde 行为总结](#serde-行为总结)
  - [返回形状差异：map vs array](#返回形状差异map-vs-array)

---

## 通用

### `HelloResponse`

健康检查响应。

**来源**：`GET /`

```rust
pub struct HelloResponse {
    pub hello: String,
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 说明 |
|------|------|-----------|------|------|
| `hello` | `String` | `hello` | ✅ | 固定值 `"mihomo"` |

**JSON 示例**：

```json
{"hello": "mihomo"}
```

---

### `VersionResponse`

版本信息响应。

**来源**：`GET /version`

```rust
pub struct VersionResponse {
    pub version: String,
    pub meta: bool,       // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 默认值 | 说明 |
|------|------|-----------|------|--------|------|
| `version` | `String` | `version` | ✅ | — | 版本字符串，如 `"v1.19.20"` |
| `meta` | `bool` | `meta` | ❌ | `false` | 是否为 Meta 分支 |

**JSON 示例**：

```json
{"version": "v1.19.20", "meta": true}
```

---

### `StatusResponse`

操作状态响应。

**来源**：`POST /restart`、`POST /upgrade`、`POST /upgrade/ui`

```rust
pub struct StatusResponse {
    pub status: String,
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 说明 |
|------|------|-----------|------|------|
| `status` | `String` | `status` | ✅ | 通常为 `"ok"` |

**JSON 示例**：

```json
{"status": "ok"}
```

---

### `ApiError`

通用错误响应。

**来源**：mihomo 在请求失败时返回（400、404、500 等）

```rust
pub struct ApiError {
    pub message: String,
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 说明 |
|------|------|-----------|------|------|
| `message` | `String` | `message` | ✅ | 错误描述文本 |

**JSON 示例**：

```json
{"message": "proxy not found"}
```

> 📌 `ApiError` 不会由 SDK 的高层方法自动返回。当你直接使用 `PipeTransport`
> 并收到非 2xx 状态码时，可以手动将 `resp.body` 反序列化为 `ApiError`。

---

## 流式数据（Streaming）

以下结构体用于流式端点，每行 JSON 反序列化为一个实例。

### `TrafficEntry`

实时流量数据。

**来源**：`GET /traffic`（流式，每秒一条）

**mihomo 来源**：`hub/route/server.go` — struct `Traffic`

```rust
pub struct TrafficEntry {
    pub up: i64,
    pub down: i64,
    pub up_total: i64,      // #[serde(default, rename = "upTotal")]
    pub down_total: i64,    // #[serde(default, rename = "downTotal")]
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 默认值 | 说明 |
|------|------|-----------|------|--------|------|
| `up` | `i64` | `up` | ✅ | — | 瞬时上行速率（字节/秒） |
| `down` | `i64` | `down` | ✅ | — | 瞬时下行速率（字节/秒） |
| `up_total` | `i64` | `upTotal` | ❌ | `0` | 累计上行总量（字节） |
| `down_total` | `i64` | `downTotal` | ❌ | `0` | 累计下行总量（字节） |

> 📌 `up_total` 和 `down_total` 在旧版 mihomo 中可能不存在（`#[serde(default)]`）。

**JSON 示例**：

```json
{"up": 1024, "down": 2048, "upTotal": 10240, "downTotal": 20480}
```

---

### `MemoryEntry`

实时内存使用数据。

**来源**：`GET /memory`（流式，每秒一条）

**mihomo 来源**：`hub/route/server.go` — struct `Memory`

```rust
pub struct MemoryEntry {
    pub inuse: u64,
    pub oslimit: u64,   // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 默认值 | 说明 |
|------|------|-----------|------|--------|------|
| `inuse` | `u64` | `inuse` | ✅ | — | 当前 Go 运行时堆内存使用量（字节） |
| `oslimit` | `u64` | `oslimit` | ❌ | `0` | OS 内存限制（字节），0 表示无限制 |

> 📌 第一条推送的 `inuse` 可能为 `0`（Go 运行时尚未更新统计数据）。

**JSON 示例**：

```json
{"inuse": 22118400, "oslimit": 0}
```

---

### `LogEntry`

日志条目（默认格式）。

**来源**：`GET /logs`（流式，事件驱动）

**mihomo 来源**：`hub/route/server.go` — struct `Log`

```rust
pub struct LogEntry {
    pub level: String,     // #[serde(rename = "type")]
    pub payload: String,
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 说明 |
|------|------|-----------|------|------|
| `level` | `String` | `type` ⚠️ | ✅ | 日志级别：`"info"` / `"warning"` / `"error"` / `"debug"` |
| `payload` | `String` | `payload` | ✅ | 日志正文 |

> ⚠️ **注意**：JSON 中的键名是 **`type`**（不是 `level`），
> Rust 侧通过 `#[serde(rename = "type")]` 映射为字段名 `level`，
> 因为 `type` 是 Rust 关键字。

**JSON 示例**：

```json
{"type": "info", "payload": "Mixed(http+socks) proxy listening at: 127.0.0.1:7890"}
```

---

### `LogStructured`

结构化日志条目。

**来源**：`GET /logs?format=structured`（流式，事件驱动）

**mihomo 来源**：`hub/route/server.go` — struct `LogStructured`

```rust
pub struct LogStructured {
    pub time: String,
    pub level: String,
    pub message: String,
    pub fields: Vec<LogStructuredField>,   // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 必填 | 默认值 | 说明 |
|------|------|-----------|------|--------|------|
| `time` | `String` | `time` | ✅ | — | 时间戳（ISO 8601） |
| `level` | `String` | `level` | ✅ | — | 日志级别 |
| `message` | `String` | `message` | ✅ | — | 日志消息正文 |
| `fields` | `Vec<LogStructuredField>` | `fields` | ❌ | `[]` | 附加键值对 |

**JSON 示例**：

```json
{
  "time": "2024-01-15T10:30:00+08:00",
  "level": "info",
  "message": "proxy listening",
  "fields": [
    {"key": "port", "value": "7890"}
  ]
}
```

---

### `LogStructuredField`

结构化日志的附加键值对。

```rust
pub struct LogStructuredField {
    pub key: String,
    pub value: String,
}
```

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `key` | `String` | `key` | 键名 |
| `value` | `String` | `value` | 值 |

---

## 配置

### `ConfigResponse`

运行时配置快照。

**来源**：`GET /configs`，由 `executor.GetGeneral()` 返回

```rust
pub struct ConfigResponse {
    pub port: i32,                                  // #[serde(default)]
    pub socks_port: i32,                            // #[serde(default, rename = "socks-port")]
    pub redir_port: i32,                            // #[serde(default, rename = "redir-port")]
    pub tproxy_port: i32,                           // #[serde(default, rename = "tproxy-port")]
    pub mixed_port: i32,                            // #[serde(default, rename = "mixed-port")]
    pub authentication: Option<Vec<String>>,         // #[serde(default)]
    pub allow_lan: bool,                            // #[serde(default, rename = "allow-lan")]
    pub bind_address: String,                       // #[serde(default, rename = "bind-address")]
    pub mode: String,                               // #[serde(default)]
    pub log_level: String,                          // #[serde(default, rename = "log-level")]
    pub ipv6: bool,                                 // #[serde(default)]
    pub sniffing: bool,                             // #[serde(default)]
    pub tcp_concurrent: bool,                       // #[serde(default, rename = "tcp-concurrent")]
    pub interface_name: String,                     // #[serde(default, rename = "interface-name")]
    pub tun: Option<TunConfig>,                     // #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,  // #[serde(flatten)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `port` | `i32` | `port` | `0` | HTTP 代理端口 |
| `socks_port` | `i32` | `socks-port` | `0` | SOCKS5 代理端口 |
| `redir_port` | `i32` | `redir-port` | `0` | 透明代理端口 |
| `tproxy_port` | `i32` | `tproxy-port` | `0` | TProxy 端口 |
| `mixed_port` | `i32` | `mixed-port` | `0` | 混合代理端口（HTTP+SOCKS） |
| `authentication` | `Option<Vec<String>>` | `authentication` | `None` | 认证用户列表 |
| `allow_lan` | `bool` | `allow-lan` | `false` | 是否允许局域网连接 |
| `bind_address` | `String` | `bind-address` | `""` | 绑定地址 |
| `mode` | `String` | `mode` | `""` | 运行模式：`"rule"` / `"global"` / `"direct"` |
| `log_level` | `String` | `log-level` | `""` | 日志级别 |
| `ipv6` | `bool` | `ipv6` | `false` | IPv6 开关 |
| `sniffing` | `bool` | `sniffing` | `false` | 嗅探开关 |
| `tcp_concurrent` | `bool` | `tcp-concurrent` | `false` | TCP 并发开关 |
| `interface_name` | `String` | `interface-name` | `""` | 出站网卡名 |
| `tun` | `Option<TunConfig>` | `tun` | `None` | TUN 配置 |
| `extra` | `HashMap<String, Value>` | *(flatten)* | `{}` | 未知/额外字段 |

> 📌 `extra` 使用 `#[serde(flatten)]`，会捕获 JSON 中所有未被显式定义的字段。
> mihomo 可能在新版本中添加新字段，它们会自动出现在 `extra` 中，而不会导致反序列化失败。

**JSON 示例**（部分）：

```json
{
  "port": 0,
  "socks-port": 0,
  "mixed-port": 7890,
  "allow-lan": false,
  "mode": "rule",
  "log-level": "info",
  "ipv6": false,
  "tun": {
    "enable": false,
    "device": "",
    "stack": "gvisor",
    "auto-route": false
  }
}
```

---

### `TunConfig`

TUN 设备配置。嵌入在 `ConfigResponse` 中。

```rust
pub struct TunConfig {
    pub enable: bool,                               // #[serde(default)]
    pub device: String,                             // #[serde(default)]
    pub stack: String,                              // #[serde(default)]
    pub dns_hijack: Option<Vec<String>>,            // #[serde(default, rename = "dns-hijack")]
    pub auto_route: bool,                           // #[serde(default, rename = "auto-route")]
    pub auto_detect_interface: bool,                // #[serde(default, rename = "auto-detect-interface")]
    pub extra: HashMap<String, serde_json::Value>,  // #[serde(flatten)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `enable` | `bool` | `enable` | `false` | TUN 是否启用 |
| `device` | `String` | `device` | `""` | TUN 设备名 |
| `stack` | `String` | `stack` | `""` | 网络栈：`"gvisor"` / `"system"` / `"lwip"` |
| `dns_hijack` | `Option<Vec<String>>` | `dns-hijack` | `None` | DNS 劫持目标列表 |
| `auto_route` | `bool` | `auto-route` | `false` | 自动路由 |
| `auto_detect_interface` | `bool` | `auto-detect-interface` | `false` | 自动检测出站网卡 |
| `extra` | `HashMap<String, Value>` | *(flatten)* | `{}` | 未知/额外字段 |

---

### `ConfigPatchRequest`

配置补丁请求体。

**来源**：`PATCH /configs`

```rust
pub struct ConfigPatchRequest(pub serde_json::Value);
```

**说明**：
一个 newtype wrapper，内部是任意 JSON `Value`。调用方通过 `serde_json::json!()` 构造。

**使用示例**：

```rust
use serde_json::json;

// SDK 内部自动包装，调用方只需传入 Value
mgr.patch_configs(json!({"mixed-port": 7890, "mode": "global"})).await?;
```

**序列化结果**：直接输出内部 Value 的 JSON 表示（透明 wrapper）。

---

### `ConfigReloadRequest`

配置重新加载请求体。

**来源**：`PUT /configs` / `PUT /configs?force=true`

```rust
pub struct ConfigReloadRequest {
    pub path: String,       // #[serde(default)]
    pub payload: String,    // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `path` | `String` | `path` | `""` | 配置文件绝对路径（空则使用当前路径） |
| `payload` | `String` | `payload` | `""` | 配置内容字符串（空则从 path 读取文件） |

**JSON 示例**：

```json
{"path": "C:\\mihomo\\config.yaml", "payload": ""}
```

---

## 代理

### `ProxiesResponse`

所有代理的响应容器。

**来源**：`GET /proxies`

```rust
pub struct ProxiesResponse {
    pub proxies: HashMap<String, ProxyInfo>,
}
```

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `proxies` | `HashMap<String, ProxyInfo>` | `proxies` | 代理名称 → 代理信息的映射 |

> ⚠️ **重要**：`GET /proxies` 返回的 `proxies` 是 **map**（`HashMap`），
> 而 `GET /group` 返回的 `proxies` 是 **array**（`Vec`）。

**JSON 示例**（部分）：

```json
{
  "proxies": {
    "DIRECT": {"name": "DIRECT", "type": "Direct", "udp": true},
    "REJECT": {"name": "REJECT", "type": "Reject"},
    "香港节点01": {"name": "香港节点01", "type": "Shadowsocks", "udp": true}
  }
}
```

---

### `ProxyInfo`

单个代理的详细信息。

**来源**：`GET /proxies`（map 中的值）、`GET /proxies/:name`

```rust
pub struct ProxyInfo {
    pub name: String,                               // #[serde(default)]
    pub proxy_type: String,                         // #[serde(default, rename = "type")]
    pub udp: bool,                                  // #[serde(default)]
    pub xudp: bool,                                 // #[serde(default)]
    pub history: Vec<ProxyDelayEntry>,              // #[serde(default)]
    pub all: Option<Vec<String>>,                   // #[serde(default)]
    pub now: Option<String>,                        // #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,  // #[serde(flatten)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `name` | `String` | `name` | `""` | 代理名称 |
| `proxy_type` | `String` | `type` | `""` | 代理类型：`"Shadowsocks"` / `"Vmess"` / `"Trojan"` / `"Selector"` / `"URLTest"` / `"Direct"` / `"Reject"` 等 |
| `udp` | `bool` | `udp` | `false` | 是否支持 UDP |
| `xudp` | `bool` | `xudp` | `false` | 是否支持 XUDP |
| `history` | `Vec<ProxyDelayEntry>` | `history` | `[]` | 延迟测试历史记录 |
| `all` | `Option<Vec<String>>` | `all` | `None` | 子代理列表（仅策略组有此字段） |
| `now` | `Option<String>` | `now` | `None` | 当前选择的代理名（仅 Selector/URLTest 等有此字段） |
| `extra` | `HashMap<String, Value>` | *(flatten)* | `{}` | 未知/额外字段 |

> 📌 `proxy_type` 的 JSON 键名为 `type`，使用 `#[serde(rename = "type")]`。
> 实际 JSON 中的字段形状因代理类型不同而变化，未被显式定义的字段会收集到 `extra` 中。

**JSON 示例**：

```json
{
  "name": "节点选择",
  "type": "Selector",
  "udp": true,
  "xudp": false,
  "history": [],
  "all": ["香港节点01", "美国节点01", "自动选择"],
  "now": "香港节点01"
}
```

---

### `SelectProxyRequest`

切换代理的请求体。

**来源**：`PUT /proxies/:name`

```rust
pub struct SelectProxyRequest {
    pub name: String,
}
```

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `name` | `String` | `name` | 要选择的目标代理名称 |

**JSON 示例**：

```json
{"name": "香港节点01"}
```

---

### `ProxyDelayEntry`

延迟测试历史记录中的单条记录。

```rust
pub struct ProxyDelayEntry {
    pub time: String,    // #[serde(default)]
    pub delay: u64,      // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `time` | `String` | `time` | `""` | 测试时间戳 |
| `delay` | `u64` | `delay` | `0` | 延迟（毫秒），0 表示超时或失败 |

**JSON 示例**：

```json
{"time": "2024-01-15T10:30:00+08:00", "delay": 120}
```

---

### `DelayResponse`

延迟测试结果。

**来源**：`GET /proxies/:name/delay`、`GET /providers/proxies/:p/:proxy/healthcheck`

```rust
pub struct DelayResponse {
    pub delay: u64,    // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `delay` | `u64` | `delay` | `0` | 延迟（毫秒），0 可能表示超时 |

**JSON 示例**：

```json
{"delay": 85}
```

---

## 策略组

### `GroupsResponse`

所有策略组的响应容器。

**来源**：`GET /group`

```rust
pub struct GroupsResponse {
    pub proxies: Vec<GroupInfo>,
}
```

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `proxies` | `Vec<GroupInfo>` | `proxies` | 策略组数组 |

> ⚠️ **重要**：虽然字段名也叫 `proxies`，但 `GET /group` 返回的是 **数组**（`Vec`），
> 而 `GET /proxies` 返回的是 **map**（`HashMap`）。

**JSON 示例**：

```json
{
  "proxies": [
    {"name": "节点选择", "type": "Selector", "now": "香港节点01", "all": ["香港节点01", "美国节点01"]},
    {"name": "自动选择", "type": "URLTest", "now": "香港节点01", "all": ["香港节点01", "美国节点01"]}
  ]
}
```

---

### `GroupInfo`

单个策略组的信息。

**来源**：`GET /group`（数组中的元素）、`GET /group/:name`

```rust
pub struct GroupInfo {
    pub name: String,                               // #[serde(default)]
    pub group_type: String,                         // #[serde(default, rename = "type")]
    pub now: String,                                // #[serde(default)]
    pub all: Vec<String>,                           // #[serde(default)]
    pub history: Vec<ProxyDelayEntry>,              // #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,  // #[serde(flatten)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `name` | `String` | `name` | `""` | 策略组名称 |
| `group_type` | `String` | `type` | `""` | 类型：`"Selector"` / `"URLTest"` / `"Fallback"` / `"LoadBalance"` / `"Relay"` |
| `now` | `String` | `now` | `""` | 当前选择的代理名 |
| `all` | `Vec<String>` | `all` | `[]` | 所有子代理名列表 |
| `history` | `Vec<ProxyDelayEntry>` | `history` | `[]` | 延迟历史 |
| `extra` | `HashMap<String, Value>` | *(flatten)* | `{}` | 未知/额外字段 |

---

### `GroupDelayResponse`

策略组批量延迟测试结果。

**来源**：`GET /group/:name/delay`

```rust
pub type GroupDelayResponse = HashMap<String, u64>;
```

**说明**：
键为代理名，值为延迟（毫秒）。

**JSON 示例**：

```json
{
  "香港节点01": 85,
  "美国节点01": 220,
  "日本节点01": 150
}
```

---

## 代理集合（Proxy Providers）

### `ProxyProvidersResponse`

所有 Proxy Provider 的响应容器。

**来源**：`GET /providers/proxies`

```rust
pub struct ProxyProvidersResponse {
    pub providers: HashMap<String, ProxyProviderInfo>,
}
```

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `providers` | `HashMap<String, ProxyProviderInfo>` | `providers` | Provider 名称 → 信息的映射 |

---

### `ProxyProviderInfo`

单个 Proxy Provider 的信息。

**来源**：`GET /providers/proxies`（map 中的值）、`GET /providers/proxies/:name`

```rust
pub struct ProxyProviderInfo {
    pub name: String,                               // #[serde(default)]
    pub provider_type: String,                      // #[serde(default, rename = "type")]
    pub vehicle_type: String,                       // #[serde(default, rename = "vehicleType")]
    pub proxies: Vec<ProxyInfo>,                    // #[serde(default)]
    pub updated_at: String,                         // #[serde(default, rename = "updatedAt")]
    pub subscription_info: Option<SubscriptionInfo>, // #[serde(default, rename = "subscriptionInfo")]
    pub extra: HashMap<String, serde_json::Value>,  // #[serde(flatten)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `name` | `String` | `name` | `""` | Provider 名称 |
| `provider_type` | `String` | `type` | `""` | Provider 类型 |
| `vehicle_type` | `String` | `vehicleType` | `""` | 数据源类型：`"HTTP"` / `"File"` |
| `proxies` | `Vec<ProxyInfo>` | `proxies` | `[]` | 包含的代理列表 |
| `updated_at` | `String` | `updatedAt` | `""` | 最后更新时间（ISO 8601） |
| `subscription_info` | `Option<SubscriptionInfo>` | `subscriptionInfo` | `None` | 订阅信息（仅 HTTP 类型有） |
| `extra` | `HashMap<String, Value>` | *(flatten)* | `{}` | 未知/额外字段 |

---

### `SubscriptionInfo`

代理集合的订阅元数据。

```rust
pub struct SubscriptionInfo {
    pub upload: u64,      // #[serde(default, rename = "Upload")]
    pub download: u64,    // #[serde(default, rename = "Download")]
    pub total: u64,       // #[serde(default, rename = "Total")]
    pub expire: u64,      // #[serde(default, rename = "Expire")]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `upload` | `u64` | `Upload` | `0` | 已上传流量（字节） |
| `download` | `u64` | `Download` | `0` | 已下载流量（字节） |
| `total` | `u64` | `Total` | `0` | 总流量额度（字节） |
| `expire` | `u64` | `Expire` | `0` | 到期时间（Unix 时间戳，秒） |

> 📌 注意 JSON 键名使用**大写首字母**（`Upload` 而非 `upload`），这是 mihomo Go 代码的风格。

**JSON 示例**：

```json
{
  "Upload": 1073741824,
  "Download": 10737418240,
  "Total": 107374182400,
  "Expire": 1735689600
}
```

---

## 规则集合（Rule Providers）

### `RuleProvidersResponse`

所有 Rule Provider 的响应容器。

**来源**：`GET /providers/rules`

```rust
pub struct RuleProvidersResponse {
    pub providers: HashMap<String, RuleProviderInfo>,
}
```

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `providers` | `HashMap<String, RuleProviderInfo>` | `providers` | Provider 名称 → 信息的映射 |

---

### `RuleProviderInfo`

单个 Rule Provider 的信息。

```rust
pub struct RuleProviderInfo {
    pub name: String,                               // #[serde(default)]
    pub provider_type: String,                      // #[serde(default, rename = "type")]
    pub behavior: String,                           // #[serde(default)]
    pub rule_count: u64,                            // #[serde(default, rename = "ruleCount")]
    pub vehicle_type: String,                       // #[serde(default, rename = "vehicleType")]
    pub updated_at: String,                         // #[serde(default, rename = "updatedAt")]
    pub extra: HashMap<String, serde_json::Value>,  // #[serde(flatten)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `name` | `String` | `name` | `""` | Provider 名称 |
| `provider_type` | `String` | `type` | `""` | Provider 类型 |
| `behavior` | `String` | `behavior` | `""` | 规则行为：`"domain"` / `"ipcidr"` / `"classical"` |
| `rule_count` | `u64` | `ruleCount` | `0` | 包含的规则数量 |
| `vehicle_type` | `String` | `vehicleType` | `""` | 数据源类型 |
| `updated_at` | `String` | `updatedAt` | `""` | 最后更新时间 |
| `extra` | `HashMap<String, Value>` | *(flatten)* | `{}` | 未知/额外字段 |

**JSON 示例**：

```json
{
  "name": "anti-ad",
  "type": "Rule",
  "behavior": "domain",
  "ruleCount": 50000,
  "vehicleType": "HTTP",
  "updatedAt": "2024-01-15T00:00:00+08:00"
}
```

---

## 规则

### `RulesResponse`

所有规则的响应容器。

**来源**：`GET /rules`

```rust
pub struct RulesResponse {
    pub rules: Vec<RuleInfo>,
}
```

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `rules` | `Vec<RuleInfo>` | `rules` | 规则数组 |

---

### `RuleInfo`

单条规则的信息。

**来源**：`hub/route/rules.go` — struct `Rule`

```rust
pub struct RuleInfo {
    pub index: i64,                   // #[serde(default)]
    pub rule_type: String,            // #[serde(default, rename = "type")]
    pub payload: String,              // #[serde(default)]
    pub proxy: String,                // #[serde(default)]
    pub size: i64,                    // #[serde(default)]
    pub extra: Option<RuleExtra>,     // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `index` | `i64` | `index` | `0` | 规则索引号 |
| `rule_type` | `String` | `type` | `""` | 规则类型：`"DOMAIN"` / `"DOMAIN-SUFFIX"` / `"IP-CIDR"` / `"GEOIP"` / `"MATCH"` 等 |
| `payload` | `String` | `payload` | `""` | 匹配规则内容 |
| `proxy` | `String` | `proxy` | `""` | 目标代理/策略组 |
| `size` | `i64` | `size` | `0` | 规则集大小（对 RULE-SET 类型有效） |
| `extra` | `Option<RuleExtra>` | `extra` | `None` | 额外元数据（命中/未命中统计） |

**JSON 示例**：

```json
{
  "index": 0,
  "type": "DOMAIN-SUFFIX",
  "payload": "google.com",
  "proxy": "节点选择",
  "size": 0,
  "extra": {
    "disabled": false,
    "hitCount": 42,
    "hitAt": "2024-01-15T10:00:00+08:00",
    "missCount": 0,
    "missAt": ""
  }
}
```

---

### `RuleExtra`

规则的额外元数据。

**来源**：`hub/route/rules.go` — struct `RuleExtra`

```rust
pub struct RuleExtra {
    pub disabled: bool,       // #[serde(default)]
    pub hit_count: u64,       // #[serde(default, rename = "hitCount")]
    pub hit_at: String,       // #[serde(default, rename = "hitAt")]
    pub miss_count: u64,      // #[serde(default, rename = "missCount")]
    pub miss_at: String,      // #[serde(default, rename = "missAt")]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `disabled` | `bool` | `disabled` | `false` | 是否被禁用 |
| `hit_count` | `u64` | `hitCount` | `0` | 命中次数 |
| `hit_at` | `String` | `hitAt` | `""` | 最后命中时间 |
| `miss_count` | `u64` | `missCount` | `0` | 未命中次数 |
| `miss_at` | `String` | `missAt` | `""` | 最后未命中时间 |

---

### `DisableRulesRequest`

规则禁用/启用请求体。

**来源**：`PATCH /rules/disable`

```rust
pub type DisableRulesRequest = HashMap<i64, bool>;
```

**说明**：
类型别名。键为规则索引号，值为是否禁用（`true` = 禁用，`false` = 启用）。

**JSON 示例**：

```json
{"0": true, "1": true, "5": false}
```

> 📌 此操作是**临时的**，重启 mihomo 后失效。

---

## 连接

### `ConnectionsResponse`

连接快照响应。

**来源**：`GET /connections`，由 `tunnel/statistic/manager.go` 的 `Snapshot()` 返回

```rust
pub struct ConnectionsResponse {
    pub download_total: u64,                      // #[serde(default, rename = "downloadTotal")]
    pub upload_total: u64,                        // #[serde(default, rename = "uploadTotal")]
    pub connections: Option<Vec<ConnectionInfo>>,  // #[serde(default)]
    pub memory: u64,                              // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `download_total` | `u64` | `downloadTotal` | `0` | 累计下行总量（字节） |
| `upload_total` | `u64` | `uploadTotal` | `0` | 累计上行总量（字节） |
| `connections` | `Option<Vec<ConnectionInfo>>` | `connections` | `None` | 活跃连接列表，可能为 `null` |
| `memory` | `u64` | `memory` | `0` | 内存使用量（字节） |

> 📌 `connections` 可能为 `null`（JSON）/ `None`（Rust），表示当前没有活跃连接。
> 使用 `as_ref().map_or(0, |c| c.len())` 安全获取连接数。

**JSON 示例**：

```json
{
  "downloadTotal": 1048576,
  "uploadTotal": 524288,
  "connections": [
    {
      "id": "abc123",
      "metadata": {
        "network": "tcp",
        "type": "HTTP",
        "sourceIP": "192.168.1.100",
        "sourcePort": "12345",
        "destinationIP": "93.184.216.34",
        "destinationPort": "443",
        "host": "example.com"
      },
      "upload": 1024,
      "download": 4096,
      "start": "2024-01-15T10:00:00+08:00",
      "chains": ["节点选择", "香港节点01"],
      "rule": "DOMAIN-SUFFIX",
      "rulePayload": "example.com"
    }
  ],
  "memory": 22118400
}
```

---

### `ConnectionInfo`

单个活跃连接的信息。

```rust
pub struct ConnectionInfo {
    pub id: String,                       // #[serde(default)]
    pub metadata: ConnectionMetadata,     // #[serde(default)]
    pub upload: u64,                      // #[serde(default)]
    pub download: u64,                    // #[serde(default)]
    pub start: String,                    // #[serde(default)]
    pub chains: Vec<String>,             // #[serde(default)]
    pub rule: String,                     // #[serde(default)]
    pub rule_payload: String,            // #[serde(default, rename = "rulePayload")]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `id` | `String` | `id` | `""` | 连接唯一 ID |
| `metadata` | `ConnectionMetadata` | `metadata` | `Default` | 连接元数据 |
| `upload` | `u64` | `upload` | `0` | 已上传字节数 |
| `download` | `u64` | `download` | `0` | 已下载字节数 |
| `start` | `String` | `start` | `""` | 连接开始时间 |
| `chains` | `Vec<String>` | `chains` | `[]` | 代理链（从策略组到最终节点） |
| `rule` | `String` | `rule` | `""` | 匹配的规则类型 |
| `rule_payload` | `String` | `rulePayload` | `""` | 匹配的规则内容 |

---

### `ConnectionMetadata`

连接的网络元数据。

```rust
#[derive(Default)]
pub struct ConnectionMetadata {
    pub network: String,           // #[serde(default)]
    pub conn_type: String,         // #[serde(default, rename = "type")]
    pub source_ip: String,         // #[serde(default, rename = "sourceIP")]
    pub destination_ip: String,    // #[serde(default, rename = "destinationIP")]
    pub source_port: String,       // #[serde(default, rename = "sourcePort")]
    pub destination_port: String,  // #[serde(default, rename = "destinationPort")]
    pub host: String,              // #[serde(default)]
    pub dns_mode: String,          // #[serde(default, rename = "dnsMode")]
    pub process_path: String,      // #[serde(default, rename = "processPath")]
    pub special_proxy: String,     // #[serde(default, rename = "specialProxy")]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `network` | `String` | `network` | `""` | 网络层协议：`"tcp"` / `"udp"` |
| `conn_type` | `String` | `type` | `""` | 连接类型：`"HTTP"` / `"HTTPS"` / `"SOCKS5"` 等 |
| `source_ip` | `String` | `sourceIP` | `""` | 源 IP |
| `destination_ip` | `String` | `destinationIP` | `""` | 目标 IP |
| `source_port` | `String` | `sourcePort` | `""` | 源端口 |
| `destination_port` | `String` | `destinationPort` | `""` | 目标端口 |
| `host` | `String` | `host` | `""` | 目标域名（嗅探或 DNS 解析得到） |
| `dns_mode` | `String` | `dnsMode` | `""` | DNS 解析模式 |
| `process_path` | `String` | `processPath` | `""` | 发起连接的进程路径 |
| `special_proxy` | `String` | `specialProxy` | `""` | 特殊代理标记 |

> 📌 `ConnectionMetadata` 实现了 `Default` trait，所有字段默认为空字符串。

---

## DNS

### `DnsQueryResponse`

DNS 查询响应。

**来源**：`GET /dns/query?name=xxx&type=A`

**mihomo 来源**：`hub/route/dns.go` — `queryDNS`

```rust
pub struct DnsQueryResponse {
    pub status: i32,                            // #[serde(default, rename = "Status")]
    pub question: Option<Vec<DnsQuestion>>,     // #[serde(default, rename = "Question")]
    pub answer: Option<Vec<DnsAnswer>>,         // #[serde(default, rename = "Answer")]
    pub authority: Option<Vec<DnsAnswer>>,      // #[serde(default, rename = "Authority")]
    pub additional: Option<Vec<DnsAnswer>>,     // #[serde(default, rename = "Additional")]
    pub tc: bool,                               // #[serde(default, rename = "TC")]
    pub rd: bool,                               // #[serde(default, rename = "RD")]
    pub ra: bool,                               // #[serde(default, rename = "RA")]
    pub ad: bool,                               // #[serde(default, rename = "AD")]
    pub cd: bool,                               // #[serde(default, rename = "CD")]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `status` | `i32` | `Status` | `0` | DNS RCODE（0=NOERROR, 3=NXDOMAIN 等） |
| `question` | `Option<Vec<DnsQuestion>>` | `Question` | `None` | 查询问题部分 |
| `answer` | `Option<Vec<DnsAnswer>>` | `Answer` | `None` | 应答部分 |
| `authority` | `Option<Vec<DnsAnswer>>` | `Authority` | `None` | 权威部分 |
| `additional` | `Option<Vec<DnsAnswer>>` | `Additional` | `None` | 附加部分 |
| `tc` | `bool` | `TC` | `false` | Truncated 标志 |
| `rd` | `bool` | `RD` | `false` | Recursion Desired 标志 |
| `ra` | `bool` | `RA` | `false` | Recursion Available 标志 |
| `ad` | `bool` | `AD` | `false` | Authenticated Data 标志 |
| `cd` | `bool` | `CD` | `false` | Checking Disabled 标志 |

> 📌 所有 JSON 键名使用**大写首字母**（Go 风格），如 `Status`、`Question`、`Answer` 等。

**JSON 示例**：

```json
{
  "Status": 0,
  "Question": [{"Name": "example.com.", "Qtype": 1, "Qclass": 1}],
  "Answer": [{"name": "example.com.", "TTL": 300, "type": 1, "data": "93.184.216.34"}],
  "TC": false,
  "RD": true,
  "RA": true,
  "AD": false,
  "CD": false
}
```

---

### `DnsQuestion`

DNS 查询的问题部分。

```rust
pub struct DnsQuestion {
    pub name: String,     // #[serde(default, rename = "Name")]
    pub qtype: u16,       // #[serde(default, rename = "Qtype")]
    pub qclass: u16,      // #[serde(default, rename = "Qclass")]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `name` | `String` | `Name` | `""` | 查询域名 |
| `qtype` | `u16` | `Qtype` | `0` | 查询类型（1=A, 28=AAAA, 15=MX 等） |
| `qclass` | `u16` | `Qclass` | `0` | 查询类（1=IN） |

---

### `DnsAnswer`

DNS 应答/权威/附加部分的单条记录。

**mihomo 来源**：`hub/route/dns.go` — `rr2Json` 映射函数

```rust
pub struct DnsAnswer {
    pub name: String,      // #[serde(default)]
    pub ttl: u64,          // #[serde(default, rename = "TTL")]
    pub rr_type: u16,      // #[serde(default, rename = "type")]
    pub data: String,      // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `name` | `String` | `name` | `""` | 域名 |
| `ttl` | `u64` | `TTL` | `0` | 生存时间（秒） |
| `rr_type` | `u16` | `type` | `0` | 记录类型（1=A, 28=AAAA, 5=CNAME 等） |
| `data` | `String` | `data` | `""` | 记录数据（IP 地址、域名等） |

> 📌 注意 `name` 和 `data` 是小写键名，而 `TTL` 是大写。这是 mihomo `rr2Json` 的实际输出。

---

## 升级

### `UpgradeRequest`

升级相关请求体。

**来源**：`POST /upgrade`、`POST /upgrade/geo`、`POST /configs/geo`

```rust
pub struct UpgradeRequest {
    pub path: String,       // #[serde(default)]
    pub payload: String,    // #[serde(default)]
}
```

| 字段 | 类型 | JSON 键名 | 默认值 | 说明 |
|------|------|-----------|--------|------|
| `path` | `String` | `path` | `""` | 升级路径 |
| `payload` | `String` | `payload` | `""` | 升级内容 |

> 📌 此结构体在当前 SDK 中主要用于内部定义，升级方法的参数通过方法签名直接传入，
> 不需要调用方手动构造 `UpgradeRequest`。

---

## 传输层

### `HttpResponse`

HTTP 响应的解析结果。

**定义位置**：`src/api/transport.rs`

```rust
#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `status` | `u16` | HTTP 状态码（200、204、400、404、500 等） |
| `body` | `String` | 响应体的原始字符串（通常是 JSON，也可能为空） |

> 📌 `HttpResponse` 不是 `serde` 结构体，不参与 JSON 序列化/反序列化。
> 它是 `PipeTransport` 的 `get`/`put`/`post`/`patch`/`delete` 方法的返回值。

详细文档见 [传输层](./transport.md#httpresponse-结构体)。

---

### `PipeStream<T>`

异步流式读取器。

**定义位置**：`src/api/stream.rs`

```rust
pub struct PipeStream<T> { /* ... */ }

impl<T: DeserializeOwned> Stream for PipeStream<T> {
    type Item = Result<T, ProcessError>;
}
```

**说明**：
`PipeStream<T>` 不是数据模型，而是 `futures_core::Stream` 的实现。
每次 `poll_next` 产出一个 `Result<T, ProcessError>`，其中 `T` 是上述流式数据类型之一。

**公开方法**：

| 方法 | 返回值 | 说明 |
|------|--------|------|
| `http_status()` | `u16` | 获取 HTTP 响应状态码（0 表示尚未解析） |

**类型参数 `T` 的典型取值**：

| 端点 | `T` |
|------|-----|
| `/traffic` | `TrafficEntry` |
| `/memory` | `MemoryEntry` |
| `/logs` | `LogEntry` |
| `/logs?format=structured` | `LogStructured` |
| `/connections` | `ConnectionsResponse` |

详细文档见 [流式读取](./streaming.md)。

---

## 设计说明

### extra 字段模式

多个模型使用了 `#[serde(flatten)]` 模式来捕获未知字段：

```rust
#[serde(flatten)]
pub extra: HashMap<String, serde_json::Value>,
```

**使用此模式的结构体**：
`ConfigResponse`、`TunConfig`、`ProxyInfo`、`GroupInfo`、`ProxyProviderInfo`、
`RuleProviderInfo`

**目的**：
- mihomo 的 API 可能在新版本中添加新的返回字段
- 使用 `flatten` 可以确保这些新字段不会导致反序列化失败
- 调用方可以通过 `extra` map 访问这些未定义的字段

**使用示例**：

```rust
let cfg = mgr.get_configs().await?;

// 访问已知字段
println!("mode: {}", cfg.mode);

// 访问可能存在的新字段
if let Some(value) = cfg.extra.get("some-new-field") {
    println!("新字段: {}", value);
}
```

---

### serde 行为总结

| 注解 | 行为 | 使用场景 |
|------|------|----------|
| `#[serde(default)]` | 字段缺失时使用类型默认值 | 几乎所有非必填字段 |
| `#[serde(rename = "xxx")]` | JSON 键名与 Rust 字段名不同时映射 | `type` → `proxy_type`、`socks-port` → `socks_port` 等 |
| `#[serde(flatten)]` | 将未知字段收集到 `HashMap` 中 | `extra` 字段 |
| `Option<T>` + `#[serde(default)]` | 字段可能缺失或为 `null` | `connections`、`all`、`now`、`tun` 等 |

**所有结构体都同时实现了 `Serialize` 和 `Deserialize`**：
- 反序列化用于解析 mihomo 的 JSON 响应
- 序列化用于构建请求体（如 `SelectProxyRequest`、`ConfigReloadRequest` 等）

---

### 返回形状差异：map vs array

mihomo API 中有一个容易混淆的设计——同名字段 `proxies` 在不同端点下返回不同的 JSON 类型：

| 端点 | 字段 `proxies` 的 JSON 类型 | Rust 类型 |
|------|---------------------------|-----------|
| `GET /proxies` | **Object**（map） | `HashMap<String, ProxyInfo>` |
| `GET /group` | **Array**（数组） | `Vec<GroupInfo>` |

SDK 通过使用不同的响应结构体（`ProxiesResponse` vs `GroupsResponse`）来正确处理这一差异。

```rust
// GET /proxies → map
let proxies: ProxiesResponse = mgr.get_proxies().await?;
for (name, info) in &proxies.proxies { /* ... */ }

// GET /group → array
let groups: GroupsResponse = mgr.get_groups().await?;
for group in &groups.proxies { /* ... */ }
```

---

## 相关文档

- [REST API 参考](./api-reference.md) — 查看各方法使用的请求/响应类型
- [流式读取](./streaming.md) — 流式数据类型的消费方式
- [错误处理](./error-handling.md) — `ProcessError` 错误类型
- [架构设计](./architecture.md) — 数据模型层在整体架构中的位置