# REST API 参考

本文档列出 `MihomoManager` 上全部 35+ 个封装方法的完整签名、参数说明、返回值、
对应 mihomo 端点和使用示例。

---

## 目录

- [健康检查 / 版本](#健康检查--版本)
  - [`hello`](#hello)
  - [`get_version`](#get_version)
- [运行配置](#运行配置)
  - [`get_configs`](#get_configs)
  - [`reload_configs`](#reload_configs)
  - [`reload_configs_no_force`](#reload_configs_no_force)
  - [`patch_configs`](#patch_configs)
  - [`update_geo_database`](#update_geo_database)
- [代理](#代理)
  - [`get_proxies`](#get_proxies)
  - [`get_proxy`](#get_proxy)
  - [`select_proxy`](#select_proxy)
  - [`test_proxy_delay`](#test_proxy_delay)
  - [`test_proxy_delay_with_expected`](#test_proxy_delay_with_expected)
  - [`unfixed_proxy`](#unfixed_proxy)
- [策略组](#策略组)
  - [`get_groups`](#get_groups)
  - [`get_group`](#get_group)
  - [`test_group_delay`](#test_group_delay)
  - [`test_group_delay_with_expected`](#test_group_delay_with_expected)
- [代理集合（Proxy Providers）](#代理集合proxy-providers)
  - [`get_proxy_providers`](#get_proxy_providers)
  - [`get_proxy_provider`](#get_proxy_provider)
  - [`update_proxy_provider`](#update_proxy_provider)
  - [`healthcheck_proxy_provider`](#healthcheck_proxy_provider)
  - [`get_proxy_in_provider`](#get_proxy_in_provider)
  - [`healthcheck_proxy_in_provider`](#healthcheck_proxy_in_provider)
- [规则](#规则)
  - [`get_rules`](#get_rules)
  - [`disable_rules`](#disable_rules)
- [规则集合（Rule Providers）](#规则集合rule-providers)
  - [`get_rule_providers`](#get_rule_providers)
  - [`update_rule_provider`](#update_rule_provider)
- [连接](#连接)
  - [`get_connections`](#get_connections)
  - [`close_all_connections`](#close_all_connections)
  - [`close_connection`](#close_connection)
- [DNS](#dns)
  - [`dns_query`](#dns_query)
- [缓存](#缓存)
  - [`flush_fakeip_cache`](#flush_fakeip_cache)
  - [`flush_dns_cache`](#flush_dns_cache)
- [重启 / 升级](#重启--升级)
  - [`restart_core`](#restart_core)
  - [`upgrade_core`](#upgrade_core)
  - [`upgrade_ui`](#upgrade_ui)
  - [`upgrade_geo`](#upgrade_geo)
- [调试](#调试)
  - [`debug_gc`](#debug_gc)
- [流式端点](#流式端点)
  - [`stream_traffic`](#stream_traffic)
  - [`stream_memory`](#stream_memory)
  - [`stream_logs`](#stream_logs)
  - [`stream_logs_structured`](#stream_logs_structured)
  - [`stream_connections`](#stream_connections)

---

## 健康检查 / 版本

### `hello`

```rust
pub async fn hello(&self) -> Result<HelloResponse, ProcessError>
```

**端点**：`GET /`

**来源**：`hub/route/server.go` — `hello` handler

**返回**：[`HelloResponse`](./models.md#helloresponse)

**说明**：
健康检查端点。mihomo 正常运行时返回 `{"hello": "mihomo"}`。
`wait_ready()` 内部使用此端点判断 API 是否就绪。

**示例**：

```rust
let resp = mgr.hello().await?;
assert_eq!(resp.hello, "mihomo");
```

---

### `get_version`

```rust
pub async fn get_version(&self) -> Result<VersionResponse, ProcessError>
```

**端点**：`GET /version`

**来源**：`hub/route/server.go` — `version` handler

**返回**：[`VersionResponse`](./models.md#versionresponse)

**说明**：
获取 mihomo 版本信息。

**示例**：

```rust
let ver = mgr.get_version().await?;
println!("版本: {}", ver.version);   // e.g. "v1.19.20"
println!("Meta 版: {}", ver.meta);   // true 表示 Meta 分支
```

---

## 运行配置

### `get_configs`

```rust
pub async fn get_configs(&self) -> Result<ConfigResponse, ProcessError>
```

**端点**：`GET /configs`

**来源**：`hub/route/configs.go` — `getConfigs`，调用 `executor.GetGeneral()`

**返回**：[`ConfigResponse`](./models.md#configresponse)

**说明**：
获取当前运行时配置的快照。包含端口号、运行模式、日志级别、TUN 配置等。
未知字段会收集到 `extra: HashMap<String, Value>` 中。

**示例**：

```rust
let cfg = mgr.get_configs().await?;
println!("mixed-port: {}", cfg.mixed_port);
println!("mode: {}", cfg.mode);
println!("log-level: {}", cfg.log_level);
println!("allow-lan: {}", cfg.allow_lan);

if let Some(ref tun) = cfg.tun {
    println!("TUN enabled: {}", tun.enable);
    println!("TUN stack: {}", tun.stack);
}
```

---

### `reload_configs`

```rust
pub async fn reload_configs(
    &self,
    path: &str,
    payload: &str,
) -> Result<(), ProcessError>
```

**端点**：`PUT /configs?force=true`

**来源**：`hub/route/configs.go` — `updateConfigs`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `path` | `&str` | 配置文件的**绝对路径**。传空字符串 `""` 使用当前加载的配置路径。 |
| `payload` | `&str` | 配置内容字符串。传空字符串 `""` 表示从 `path` 读取文件。 |

**返回**：`Ok(())` 或错误

**注意事项**：
- `path` 如果不为空，**必须是绝对路径**
- `path` 必须在 mihomo 的 `SAFE_PATHS` 列表中（安全策略）
- `?force=true` 会强制重新加载，即使配置内容未变
- 重新加载会导致短暂的服务中断（端口重新绑定等）

**示例**：

```rust
// 从当前路径重新加载
mgr.reload_configs("", "").await?;

// 从指定路径加载
mgr.reload_configs("C:\\mihomo\\config2.yaml", "").await?;

// 直接传入配置内容
let yaml = "mixed-port: 7890\nmode: rule\n";
mgr.reload_configs("", yaml).await?;
```

---

### `reload_configs_no_force`

```rust
pub async fn reload_configs_no_force(
    &self,
    path: &str,
    payload: &str,
) -> Result<(), ProcessError>
```

**端点**：`PUT /configs`（不带 `?force=true`）

**来源**：`hub/route/configs.go` — `updateConfigs`

**说明**：
与 `reload_configs` 相同，但不带 `?force=true` 参数。
如果配置内容未变，mihomo 可能跳过重新加载。

**参数**：同 [`reload_configs`](#reload_configs)。

---

### `patch_configs`

```rust
pub async fn patch_configs(
    &self,
    patch: serde_json::Value,
) -> Result<(), ProcessError>
```

**端点**：`PATCH /configs`

**来源**：`hub/route/configs.go` — `patchConfigs`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `patch` | `serde_json::Value` | 要修改的配置字段（任意 JSON 对象） |

**返回**：`Ok(())` 或错误

**说明**：
动态修改部分运行时配置字段。只修改传入的字段，不影响其他配置。
修改立即生效，无需重启。

**支持的字段**（源自 `configSchema` 结构体定义）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `mixed-port` | `int` | 混合代理端口 |
| `port` | `int` | HTTP 代理端口 |
| `socks-port` | `int` | SOCKS5 代理端口 |
| `redir-port` | `int` | 透明代理端口 |
| `tproxy-port` | `int` | TProxy 端口 |
| `mode` | `string` | 运行模式：`"rule"` / `"global"` / `"direct"` |
| `log-level` | `string` | 日志级别：`"debug"` / `"info"` / `"warning"` / `"error"` / `"silent"` |
| `allow-lan` | `bool` | 是否允许局域网连接 |
| `bind-address` | `string` | 绑定地址 |
| `sniffing` | `bool` | 嗅探开关 |
| `tcp-concurrent` | `bool` | TCP 并发开关 |
| `interface-name` | `string` | 出站网卡名 |
| `tun` | `object` | TUN 配置对象 |

**示例**：

```rust
use serde_json::json;

// 修改端口
mgr.patch_configs(json!({"mixed-port": 8890})).await?;

// 切换模式
mgr.patch_configs(json!({"mode": "global"})).await?;

// 同时修改多个字段
mgr.patch_configs(json!({
    "mixed-port": 7890,
    "mode": "rule",
    "allow-lan": true,
    "log-level": "info"
})).await?;

// 启用 TUN
mgr.patch_configs(json!({
    "tun": {
        "enable": true,
        "stack": "gvisor",
        "auto-route": true
    }
})).await?;
```

---

### `update_geo_database`

```rust
pub async fn update_geo_database(&self) -> Result<(), ProcessError>
```

**端点**：`POST /configs/geo`

**来源**：`hub/route/configs.go` — `updateGeoDatabases`

**说明**：
触发更新 GeoIP / GeoSite 数据库。请求体会被忽略，
实际调用的是 `updater.UpdateGeoDatabases()`。

**示例**：

```rust
mgr.update_geo_database().await?;
println!("GEO 数据库更新完成");
```

---

## 代理

### `get_proxies`

```rust
pub async fn get_proxies(&self) -> Result<ProxiesResponse, ProcessError>
```

**端点**：`GET /proxies`

**来源**：`hub/route/proxies.go` — `getProxies`

**返回**：[`ProxiesResponse`](./models.md#proxiesresponse)

**说明**：
获取所有代理信息，包括配置文件中定义的代理和 Provider 提供的代理。
返回值是一个 **map**：`{"proxies": {"代理名": ProxyInfo, ...}}`。

> ⚠️ 注意：`GET /proxies` 返回的 `proxies` 是 **HashMap**（map），
> 而 `GET /group` 返回的 `proxies` 是 **Vec**（数组）。接口形状不同。

**示例**：

```rust
let resp = mgr.get_proxies().await?;
for (name, info) in &resp.proxies {
    println!("{}: type={}, udp={}", name, info.proxy_type, info.udp);
    if let Some(ref all) = info.all {
        println!("  子代理: {:?}", all);
    }
}
```

---

### `get_proxy`

```rust
pub async fn get_proxy(&self, name: &str) -> Result<ProxyInfo, ProcessError>
```

**端点**：`GET /proxies/:name`

**来源**：`hub/route/proxies.go` — `getProxy`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 代理名称（会自动 URL 编码） |

**返回**：[`ProxyInfo`](./models.md#proxyinfo)

**说明**：
获取指定名称的代理详细信息。如果代理不存在，mihomo 返回 404。

**示例**：

```rust
let proxy = mgr.get_proxy("香港节点01").await?;
println!("类型: {}", proxy.proxy_type);
println!("UDP: {}", proxy.udp);
for entry in &proxy.history {
    println!("  延迟记录: time={}, delay={}ms", entry.time, entry.delay);
}
```

---

### `select_proxy`

```rust
pub async fn select_proxy(
    &self,
    group: &str,
    proxy: &str,
) -> Result<(), ProcessError>
```

**端点**：`PUT /proxies/:name`

**来源**：`hub/route/proxies.go` — `updateProxy`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `group` | `&str` | Selector 策略组名称 |
| `proxy` | `&str` | 要选择的目标代理名称 |

**返回**：`Ok(())` 或错误

**说明**：
在 `Selector` 类型的策略组中切换当前使用的代理。

- 目标代理必须是 `SelectAble` 类型（Selector、URLTest、Fallback 等），否则返回 400
- 目标代理名必须是该策略组的子代理之一

**请求体**：`{"name": "proxy_name"}`

**示例**：

```rust
// 在"节点选择"策略组中选择"香港节点01"
mgr.select_proxy("节点选择", "香港节点01").await?;
println!("已切换到香港节点01");

// 切换到自动选择
mgr.select_proxy("节点选择", "自动选择").await?;
```

---

### `test_proxy_delay`

```rust
pub async fn test_proxy_delay(
    &self,
    name: &str,
    url: &str,
    timeout_ms: u64,
) -> Result<DelayResponse, ProcessError>
```

**端点**：`GET /proxies/:name/delay?url=xxx&timeout=5000`

**来源**：`hub/route/proxies.go` — `getProxyDelay`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 代理名称 |
| `url` | `&str` | 测试目标 URL，如 `"https://www.google.com"` |
| `timeout_ms` | `u64` | 超时时间（毫秒） |

**返回**：[`DelayResponse`](./models.md#delayresponse) — `{"delay": N}`

**说明**：
通过指定代理访问目标 URL 并测量延迟。超时或连接失败时 `delay` 可能为 0 或 API 返回错误。

**示例**：

```rust
let resp = mgr.test_proxy_delay(
    "香港节点01",
    "https://www.google.com",
    5000,
).await?;
println!("延迟: {}ms", resp.delay);
```

---

### `test_proxy_delay_with_expected`

```rust
pub async fn test_proxy_delay_with_expected(
    &self,
    name: &str,
    url: &str,
    timeout_ms: u64,
    expected: &str,
) -> Result<DelayResponse, ProcessError>
```

**端点**：`GET /proxies/:name/delay?url=xxx&timeout=5000&expected=200`

**来源**：`hub/route/proxies.go` — `getProxyDelay`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 代理名称 |
| `url` | `&str` | 测试目标 URL |
| `timeout_ms` | `u64` | 超时时间（毫秒） |
| `expected` | `&str` | 期望的 HTTP 状态码范围，如 `"200"` 或 `"200-299,304"` |

**返回**：[`DelayResponse`](./models.md#delayresponse)

**说明**：
与 `test_proxy_delay` 相同，但额外检查目标 URL 返回的 HTTP 状态码是否在 `expected` 范围内。
如果状态码不匹配，视为测试失败。

**示例**：

```rust
let resp = mgr.test_proxy_delay_with_expected(
    "香港节点01",
    "https://www.google.com",
    5000,
    "200-299",
).await?;
println!("延迟: {}ms", resp.delay);
```

---

### `unfixed_proxy`

```rust
pub async fn unfixed_proxy(&self, name: &str) -> Result<(), ProcessError>
```

**端点**：`DELETE /proxies/:name`

**来源**：`hub/route/proxies.go` — `unfixedProxy`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 策略组名称 |

**说明**：
清除非 Selector 类型策略组（如 URLTest、Fallback）的 fixed 选择，恢复自动选择行为。

- 仅对 `SelectAble` 且**非** `Selector` 类型的策略组生效
- 对 `Selector` 类型无效（Selector 的选择通过 `select_proxy` 控制）

**示例**：

```rust
// 清除 "自动选择"（URLTest 类型）的固定节点
mgr.unfixed_proxy("自动选择").await?;
```

---

## 策略组

### `get_groups`

```rust
pub async fn get_groups(&self) -> Result<GroupsResponse, ProcessError>
```

**端点**：`GET /group`

**来源**：`hub/route/groups.go` — `getGroups`

**返回**：[`GroupsResponse`](./models.md#groupsresponse)

**说明**：
获取所有策略组信息。

> ⚠️ **重要**：返回的 `proxies` 字段是 **数组**（`Vec<GroupInfo>`），
> 不是 map！这与 `GET /proxies` 返回的 `HashMap<String, ProxyInfo>` 不同。

**示例**：

```rust
let groups = mgr.get_groups().await?;
for group in &groups.proxies {
    println!(
        "策略组: {} ({}), 当前: {}, 子代理: {:?}",
        group.name, group.group_type, group.now, group.all
    );
}
```

---

### `get_group`

```rust
pub async fn get_group(&self, name: &str) -> Result<GroupInfo, ProcessError>
```

**端点**：`GET /group/:name`

**来源**：`hub/route/groups.go` — `getGroup`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 策略组名称 |

**返回**：[`GroupInfo`](./models.md#groupinfo)

**说明**：
获取指定策略组的信息。如果目标代理不是 ProxyGroup 类型，返回 404。

**示例**：

```rust
let group = mgr.get_group("节点选择").await?;
println!("类型: {}", group.group_type);
println!("当前选择: {}", group.now);
println!("所有子代理: {:?}", group.all);
```

---

### `test_group_delay`

```rust
pub async fn test_group_delay(
    &self,
    name: &str,
    url: &str,
    timeout_ms: u64,
) -> Result<GroupDelayResponse, ProcessError>
```

**端点**：`GET /group/:name/delay?url=xxx&timeout=5000`

**来源**：`hub/route/groups.go` — `getGroupDelay`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 策略组名称 |
| `url` | `&str` | 测试目标 URL |
| `timeout_ms` | `u64` | 超时时间（毫秒） |

**返回**：[`GroupDelayResponse`](./models.md#groupdelayresponse) — `HashMap<String, u64>`

**说明**：
同时测试策略组内所有节点的延迟。返回一个 map：`{"代理名": 延迟ms, ...}`。

- 会同时清除自动策略组（URLTest/Fallback）的 fixed 选择
- 并发执行所有节点的延迟测试

**示例**：

```rust
let delays = mgr.test_group_delay(
    "节点选择",
    "https://www.google.com",
    5000,
).await?;

for (name, delay) in &delays {
    println!("{}: {}ms", name, delay);
}
```

---

### `test_group_delay_with_expected`

```rust
pub async fn test_group_delay_with_expected(
    &self,
    name: &str,
    url: &str,
    timeout_ms: u64,
    expected: &str,
) -> Result<GroupDelayResponse, ProcessError>
```

**端点**：`GET /group/:name/delay?url=xxx&timeout=5000&expected=200`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 策略组名称 |
| `url` | `&str` | 测试目标 URL |
| `timeout_ms` | `u64` | 超时时间（毫秒） |
| `expected` | `&str` | 期望状态码范围 |

**返回**：[`GroupDelayResponse`](./models.md#groupdelayresponse)

**说明**：
与 `test_group_delay` 相同，附带 `expected` 状态码过滤。

---

## 代理集合（Proxy Providers）

### `get_proxy_providers`

```rust
pub async fn get_proxy_providers(&self) -> Result<ProxyProvidersResponse, ProcessError>
```

**端点**：`GET /providers/proxies`

**来源**：`hub/route/provider.go` — `getProviders`

**返回**：[`ProxyProvidersResponse`](./models.md#proxyprovidersresponse)

**说明**：
获取所有 Proxy Provider 的信息。返回格式为 map：
`{"providers": {"provider名": ProxyProviderInfo, ...}}`。

**示例**：

```rust
let resp = mgr.get_proxy_providers().await?;
for (name, provider) in &resp.providers {
    println!(
        "Provider: {} (type={}, vehicle={})",
        name, provider.provider_type, provider.vehicle_type
    );
    println!("  代理数: {}", provider.proxies.len());
    println!("  更新时间: {}", provider.updated_at);
    if let Some(ref sub) = provider.subscription_info {
        println!("  订阅: upload={}, download={}, total={}, expire={}",
            sub.upload, sub.download, sub.total, sub.expire);
    }
}
```

---

### `get_proxy_provider`

```rust
pub async fn get_proxy_provider(
    &self,
    name: &str,
) -> Result<ProxyProviderInfo, ProcessError>
```

**端点**：`GET /providers/proxies/:name`

**来源**：`hub/route/provider.go` — `getProvider`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | Provider 名称 |

**返回**：[`ProxyProviderInfo`](./models.md#proxyproviderinfo)

---

### `update_proxy_provider`

```rust
pub async fn update_proxy_provider(&self, name: &str) -> Result<(), ProcessError>
```

**端点**：`PUT /providers/proxies/:name`

**来源**：`hub/route/provider.go` — `updateProvider`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | Provider 名称 |

**说明**：
触发 Provider 的 `Update()` 方法，从远程/本地源拉取最新的代理列表。

**示例**：

```rust
mgr.update_proxy_provider("我的订阅").await?;
println!("订阅更新完成");
```

---

### `healthcheck_proxy_provider`

```rust
pub async fn healthcheck_proxy_provider(&self, name: &str) -> Result<(), ProcessError>
```

**端点**：`GET /providers/proxies/:name/healthcheck`

**来源**：`hub/route/provider.go` — `healthCheckProvider`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | Provider 名称 |

**说明**：
触发指定 Provider 的健康检查，测试其中所有代理的可用性。

**示例**：

```rust
mgr.healthcheck_proxy_provider("我的订阅").await?;
println!("健康检查完成");
```

---

### `get_proxy_in_provider`

```rust
pub async fn get_proxy_in_provider(
    &self,
    provider: &str,
    proxy: &str,
) -> Result<ProxyInfo, ProcessError>
```

**端点**：`GET /providers/proxies/:provider/:proxy`

**来源**：`hub/route/provider.go` — `proxyProviderProxyRouter` mounts `getProxy`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `provider` | `&str` | Provider 名称 |
| `proxy` | `&str` | Provider 内的代理名称 |

**返回**：[`ProxyInfo`](./models.md#proxyinfo)

**说明**：
获取指定 Provider 内的特定代理的详细信息。

**示例**：

```rust
let proxy = mgr.get_proxy_in_provider("我的订阅", "香港节点01").await?;
println!("类型: {}, UDP: {}", proxy.proxy_type, proxy.udp);
```

---

### `healthcheck_proxy_in_provider`

```rust
pub async fn healthcheck_proxy_in_provider(
    &self,
    provider: &str,
    proxy: &str,
    url: &str,
    timeout_ms: u64,
) -> Result<DelayResponse, ProcessError>
```

**端点**：`GET /providers/proxies/:provider/:proxy/healthcheck?url=xxx&timeout=5000`

**来源**：`hub/route/provider.go` — `proxyProviderProxyRouter` mounts `getProxyDelay`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `provider` | `&str` | Provider 名称 |
| `proxy` | `&str` | Provider 内的代理名称 |
| `url` | `&str` | 测试目标 URL |
| `timeout_ms` | `u64` | 超时时间（毫秒） |

**返回**：[`DelayResponse`](./models.md#delayresponse)

**说明**：
测试 Provider 内指定代理的延迟。复用 `proxies.go` 的 `getProxyDelay` handler。

**示例**：

```rust
let resp = mgr.healthcheck_proxy_in_provider(
    "我的订阅",
    "香港节点01",
    "https://www.google.com",
    5000,
).await?;
println!("延迟: {}ms", resp.delay);
```

---

## 规则

### `get_rules`

```rust
pub async fn get_rules(&self) -> Result<RulesResponse, ProcessError>
```

**端点**：`GET /rules`

**来源**：`hub/route/rules.go` — `getRules`

**返回**：[`RulesResponse`](./models.md#rulesresponse)

**说明**：
获取当前加载的所有规则。每条规则包含索引、类型、匹配规则、目标代理、规则集大小等信息。

**示例**：

```rust
let resp = mgr.get_rules().await?;
println!("共 {} 条规则", resp.rules.len());
for rule in &resp.rules {
    println!(
        "  #{}: {} {} → {}",
        rule.index, rule.rule_type, rule.payload, rule.proxy
    );
    if let Some(ref extra) = rule.extra {
        println!("    命中: {} 次, 未命中: {} 次", extra.hit_count, extra.miss_count);
    }
}
```

---

### `disable_rules`

```rust
pub async fn disable_rules(
    &self,
    rules: &DisableRulesRequest,
) -> Result<(), ProcessError>
```

**端点**：`PATCH /rules/disable`

**来源**：`hub/route/rules.go` — `disableRules`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `rules` | `&DisableRulesRequest` | `HashMap<i64, bool>`：规则索引 → 是否禁用 |

**说明**：
批量禁用或启用指定规则。

- key 是规则的 `index`（从 `get_rules` 获取）
- value 为 `true` 表示禁用，`false` 表示启用
- **此操作是临时的**，重启 mihomo 后失效

**示例**：

```rust
use std::collections::HashMap;

let mut rules = HashMap::new();
rules.insert(0, true);   // 禁用第 0 条规则
rules.insert(1, true);   // 禁用第 1 条规则
rules.insert(5, false);  // 启用第 5 条规则

mgr.disable_rules(&rules).await?;
```

---

## 规则集合（Rule Providers）

### `get_rule_providers`

```rust
pub async fn get_rule_providers(&self) -> Result<RuleProvidersResponse, ProcessError>
```

**端点**：`GET /providers/rules`

**来源**：`hub/route/provider.go` — `getRuleProviders`

**返回**：[`RuleProvidersResponse`](./models.md#ruleprovidersresponse)

**说明**：
获取所有 Rule Provider 信息。返回格式为 map：
`{"providers": {"provider名": RuleProviderInfo, ...}}`。

**示例**：

```rust
let resp = mgr.get_rule_providers().await?;
for (name, provider) in &resp.providers {
    println!(
        "Rule Provider: {} (type={}, behavior={}, rules={})",
        name, provider.provider_type, provider.behavior, provider.rule_count
    );
}
```

---

### `update_rule_provider`

```rust
pub async fn update_rule_provider(&self, name: &str) -> Result<(), ProcessError>
```

**端点**：`PUT /providers/rules/:name`

**来源**：`hub/route/provider.go` — `updateRuleProvider`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | Rule Provider 名称 |

**说明**：
触发指定 Rule Provider 的更新。

**示例**：

```rust
mgr.update_rule_provider("anti-ad").await?;
```

---

## 连接

### `get_connections`

```rust
pub async fn get_connections(&self) -> Result<ConnectionsResponse, ProcessError>
```

**端点**：`GET /connections`

**来源**：`hub/route/connections.go` — `getConnections`（非 WebSocket 路径）

**返回**：[`ConnectionsResponse`](./models.md#connectionsresponse)

**说明**：
获取当前所有活跃连接的快照。包括累计上下行流量、连接列表和内存使用。

> 📌 `connections` 字段可能为 `null`（即 `None`），表示当前没有活跃连接。

**示例**：

```rust
let resp = mgr.get_connections().await?;
println!("累计: ↓{} ↑{}", resp.download_total, resp.upload_total);

if let Some(ref conns) = resp.connections {
    println!("活跃连接: {} 个", conns.len());
    for conn in conns {
        println!(
            "  [{}] {} {}:{} → {}:{} via {} (↓{} ↑{})",
            conn.id,
            conn.metadata.network,
            conn.metadata.source_ip,
            conn.metadata.source_port,
            conn.metadata.host,
            conn.metadata.destination_port,
            conn.rule,
            conn.download,
            conn.upload,
        );
    }
} else {
    println!("无活跃连接");
}
```

---

### `close_all_connections`

```rust
pub async fn close_all_connections(&self) -> Result<(), ProcessError>
```

**端点**：`DELETE /connections`

**来源**：`hub/route/connections.go` — `closeAllConnections`

**说明**：
关闭所有活跃连接。

**示例**：

```rust
mgr.close_all_connections().await?;
println!("所有连接已关闭");
```

---

### `close_connection`

```rust
pub async fn close_connection(&self, id: &str) -> Result<(), ProcessError>
```

**端点**：`DELETE /connections/:id`

**来源**：`hub/route/connections.go` — `closeConnection`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | `&str` | 连接 ID（从 `get_connections` 获取） |

**说明**：
关闭指定 ID 的连接。

**示例**：

```rust
// 关闭第一个连接
let resp = mgr.get_connections().await?;
if let Some(ref conns) = resp.connections {
    if let Some(first) = conns.first() {
        mgr.close_connection(&first.id).await?;
        println!("已关闭连接 {}", first.id);
    }
}
```

---

## DNS

### `dns_query`

```rust
pub async fn dns_query(
    &self,
    name: &str,
    query_type: &str,
) -> Result<DnsQueryResponse, ProcessError>
```

**端点**：`GET /dns/query?name=xxx&type=A`

**来源**：`hub/route/dns.go` — `queryDNS`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `&str` | 查询的域名，如 `"example.com"` |
| `query_type` | `&str` | DNS 记录类型：`"A"`、`"AAAA"`、`"MX"`、`"CNAME"`、`"TXT"` 等。传空字符串 `""` 默认使用 `"A"`。 |

**返回**：[`DnsQueryResponse`](./models.md#dnsqueryresponse)

**说明**：
通过 mihomo 内置的 DNS 解析器执行 DNS 查询。

**示例**：

```rust
// 查询 A 记录
let resp = mgr.dns_query("example.com", "A").await?;
println!("状态码: {}", resp.status);
if let Some(ref answers) = resp.answer {
    for ans in answers {
        println!("  {} TTL={} type={} data={}", ans.name, ans.ttl, ans.rr_type, ans.data);
    }
}

// 查询 AAAA 记录
let resp = mgr.dns_query("google.com", "AAAA").await?;

// 使用默认类型（A）
let resp = mgr.dns_query("cloudflare.com", "").await?;
```

---

## 缓存

### `flush_fakeip_cache`

```rust
pub async fn flush_fakeip_cache(&self) -> Result<(), ProcessError>
```

**端点**：`POST /cache/fakeip/flush`

**来源**：`hub/route/cache.go` — `flushFakeIPPool`

**说明**：
清除 FakeIP 地址池缓存。当使用 FakeIP 模式时，此操作会重置所有已分配的虚拟 IP 映射。

**示例**：

```rust
mgr.flush_fakeip_cache().await?;
println!("FakeIP 缓存已清除");
```

---

### `flush_dns_cache`

```rust
pub async fn flush_dns_cache(&self) -> Result<(), ProcessError>
```

**端点**：`POST /cache/dns/flush`

**来源**：`hub/route/cache.go` — `flushDnsCache`

**说明**：
清除 DNS 解析缓存。所有缓存的 DNS 记录会被清空，后续查询会重新向上游 DNS 服务器请求。

**示例**：

```rust
mgr.flush_dns_cache().await?;
println!("DNS 缓存已清除");
```

---

## 重启 / 升级

### `restart_core`

```rust
pub async fn restart_core(&self) -> Result<StatusResponse, ProcessError>
```

**端点**：`POST /restart`

**来源**：`hub/route/restart.go` — `restart`

**返回**：[`StatusResponse`](./models.md#statusresponse) — `{"status": "ok"}`

**说明**：
重启 mihomo 内核进程。

> ⚠️ **重要注意事项**：
>
> 1. 此操作会导致 mihomo 进程通过 `exec` 重启自身
> 2. **在 Windows 上**，mihomo 会启动一个新进程然后 `os.Exit(0)`
> 3. 调用后当前 pipe 连接**会断开**
> 4. 新进程的 PID 会改变
> 5. `MihomoManager` 内部持有的 `child` 会变为"已退出"状态
> 6. 需要重新调用 `wait_ready()` 等待新进程 API 就绪
>
> **推荐**：如果你通过 `MihomoManager` 管理进程，使用 `mgr.restart()` 而非此方法。
> `mgr.restart()` 会正确处理进程句柄的更新。

**示例**：

```rust
let resp = mgr.restart_core().await?;
assert_eq!(resp.status, "ok");
// 连接已断开，需要等待新进程就绪
tokio::time::sleep(Duration::from_secs(2)).await;
mgr.wait_ready(20, Duration::from_millis(500)).await?;
```

---

### `upgrade_core`

```rust
pub async fn upgrade_core(
    &self,
    channel: Option<&str>,
    force: bool,
) -> Result<StatusResponse, ProcessError>
```

**端点**：`POST /upgrade[?channel=xxx&force=true]`

**来源**：`hub/route/upgrade.go` — `upgradeCore`

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `channel` | `Option<&str>` | 更新通道（可选），如 `Some("alpha")`。`None` 使用默认通道。 |
| `force` | `bool` | 是否强制更新（即使已是最新版本） |

**返回**：[`StatusResponse`](./models.md#statusresponse)

**说明**：
下载并更新 mihomo 内核二进制。成功后会自动调用 `restartExecutable` 重启。

> ⚠️ 更新成功后进程会重启，pipe 连接断开。

**示例**：

```rust
// 默认通道更新
let resp = mgr.upgrade_core(None, false).await?;

// 强制更新到 alpha 通道
let resp = mgr.upgrade_core(Some("alpha"), true).await?;
```

---

### `upgrade_ui`

```rust
pub async fn upgrade_ui(&self) -> Result<StatusResponse, ProcessError>
```

**端点**：`POST /upgrade/ui`

**来源**：`hub/route/upgrade.go` — `updateUI`

**返回**：[`StatusResponse`](./models.md#statusresponse)

**说明**：
更新外部 UI 面板。需要配置文件中设置了 `external-ui` 目录。

**示例**：

```rust
let resp = mgr.upgrade_ui().await?;
println!("UI 更新: {}", resp.status);
```

---

### `upgrade_geo`

```rust
pub async fn upgrade_geo(&self) -> Result<(), ProcessError>
```

**端点**：`POST /upgrade/geo`

**来源**：`hub/route/upgrade.go` — 复用 `updateGeoDatabases` handler

**说明**：
通过 upgrade 路径更新 GEO 数据库。功能上与 `update_geo_database()` 相同。

**示例**：

```rust
mgr.upgrade_geo().await?;
```

---

## 调试

### `debug_gc`

```rust
pub async fn debug_gc(&self) -> Result<(), ProcessError>
```

**端点**：`PUT /debug/gc`

**来源**：`hub/route/server.go` — debug router, `/gc` handler

**说明**：
手动触发 Go 运行时的 GC（`debug.FreeOSMemory`），释放不再使用的内存给操作系统。

> ⚠️ **前置条件**：mihomo 必须以 `log-level: debug` 启动，否则 `/debug` 路径不可用。

**示例**：

```rust
mgr.debug_gc().await?;
println!("GC 完成");
```

---

## 流式端点

以下方法返回 `PipeStream<T>`，实现了 `futures_core::Stream` trait，
用于持续接收实时推送数据。详细使用文档见 [流式读取](./streaming.md)。

### `stream_traffic`

```rust
pub async fn stream_traffic(&self) -> Result<PipeStream<TrafficEntry>, ProcessError>
```

**端点**：`GET /traffic`

**来源**：`hub/route/server.go` — `traffic` handler

**推送频率**：约每 1 秒

**数据类型**：[`TrafficEntry`](./models.md#trafficentry)

**说明**：
流式订阅实时流量数据。每条包含瞬时上下行速率和累计总量。

**示例**：

```rust
use futures_core::Stream;
use std::pin::pin;
use std::future::poll_fn;
use std::task::Poll;

let stream = mgr.stream_traffic().await?;
let mut pinned = pin!(stream);
for _ in 0..5 {
    if let Some(Ok(e)) = poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
        println!("↑ {} B/s  ↓ {} B/s", e.up, e.down);
    }
}
```

---

### `stream_memory`

```rust
pub async fn stream_memory(&self) -> Result<PipeStream<MemoryEntry>, ProcessError>
```

**端点**：`GET /memory`

**来源**：`hub/route/server.go` — `memory` handler

**推送频率**：约每 1 秒

**数据类型**：[`MemoryEntry`](./models.md#memoryentry)

**说明**：
流式订阅实时内存使用数据。

**示例**：

```rust
let stream = mgr.stream_memory().await?;
let mut pinned = pin!(stream);
if let Some(Ok(e)) = poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
    println!("内存使用: {:.2} MB", e.inuse as f64 / 1024.0 / 1024.0);
}
```

---

### `stream_logs`

```rust
pub async fn stream_logs(
    &self,
    level: &str,
) -> Result<PipeStream<LogEntry>, ProcessError>
```

**端点**：`GET /logs[?level=xxx]`

**来源**：`hub/route/server.go` — `getLogs` handler

**推送频率**：事件驱动

**数据类型**：[`LogEntry`](./models.md#logentry)

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `level` | `&str` | 最低日志级别：`"debug"` / `"info"` / `"warning"` / `"error"` / `"silent"`。传 `""` 不过滤。 |

**示例**：

```rust
let stream = mgr.stream_logs("info").await?;
let mut pinned = pin!(stream);
while let Some(Ok(e)) = poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
    println!("[{}] {}", e.level, e.payload);
}
```

---

### `stream_logs_structured`

```rust
pub async fn stream_logs_structured(
    &self,
    level: &str,
) -> Result<PipeStream<LogStructured>, ProcessError>
```

**端点**：`GET /logs?format=structured[&level=xxx]`

**来源**：`hub/route/server.go` — `getLogs` handler（`format=structured` 分支）

**推送频率**：事件驱动

**数据类型**：[`LogStructured`](./models.md#logstructured)

**参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `level` | `&str` | 最低日志级别。传 `""` 不过滤。 |

**示例**：

```rust
let stream = mgr.stream_logs_structured("debug").await?;
let mut pinned = pin!(stream);
while let Some(Ok(e)) = poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
    println!("[{}] {} {}", e.time, e.level, e.message);
}
```

---

### `stream_connections`

```rust
pub async fn stream_connections(
    &self,
) -> Result<PipeStream<ConnectionsResponse>, ProcessError>
```

**端点**：`GET /connections`（流式路径）

**来源**：`hub/route/connections.go` — `getConnections`

**推送频率**：事件驱动

**数据类型**：[`ConnectionsResponse`](./models.md#connectionsresponse)

**说明**：
流式订阅连接快照。

> ⚠️ **限制**：Named Pipe 不支持 WebSocket 升级，mihomo 的 `/connections` 在 pipe
> 上通常只返回一次快照后关闭。建议使用 `get_connections()` 配合定时轮询。
> 详见 [流式读取 — 关于 /connections 端点](./streaming.md#关于-connections-端点)。

**示例**：

```rust
// 推荐方式：定时轮询
loop {
    let snap = mgr.get_connections().await?;
    let count = snap.connections.as_ref().map_or(0, |c| c.len());
    println!("活跃连接: {}", count);
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

---

## 方法速查表

| 方法 | HTTP | 端点 | 返回类型 |
|------|------|------|----------|
| `hello` | GET | `/` | `HelloResponse` |
| `get_version` | GET | `/version` | `VersionResponse` |
| `get_configs` | GET | `/configs` | `ConfigResponse` |
| `reload_configs` | PUT | `/configs?force=true` | `()` |
| `reload_configs_no_force` | PUT | `/configs` | `()` |
| `patch_configs` | PATCH | `/configs` | `()` |
| `update_geo_database` | POST | `/configs/geo` | `()` |
| `get_proxies` | GET | `/proxies` | `ProxiesResponse` |
| `get_proxy` | GET | `/proxies/:name` | `ProxyInfo` |
| `select_proxy` | PUT | `/proxies/:name` | `()` |
| `test_proxy_delay` | GET | `/proxies/:name/delay` | `DelayResponse` |
| `test_proxy_delay_with_expected` | GET | `/proxies/:name/delay` | `DelayResponse` |
| `unfixed_proxy` | DELETE | `/proxies/:name` | `()` |
| `get_groups` | GET | `/group` | `GroupsResponse` |
| `get_group` | GET | `/group/:name` | `GroupInfo` |
| `test_group_delay` | GET | `/group/:name/delay` | `GroupDelayResponse` |
| `test_group_delay_with_expected` | GET | `/group/:name/delay` | `GroupDelayResponse` |
| `get_proxy_providers` | GET | `/providers/proxies` | `ProxyProvidersResponse` |
| `get_proxy_provider` | GET | `/providers/proxies/:name` | `ProxyProviderInfo` |
| `update_proxy_provider` | PUT | `/providers/proxies/:name` | `()` |
| `healthcheck_proxy_provider` | GET | `/providers/proxies/:name/healthcheck` | `()` |
| `get_proxy_in_provider` | GET | `/providers/proxies/:p/:proxy` | `ProxyInfo` |
| `healthcheck_proxy_in_provider` | GET | `/providers/proxies/:p/:proxy/healthcheck` | `DelayResponse` |
| `get_rules` | GET | `/rules` | `RulesResponse` |
| `disable_rules` | PATCH | `/rules/disable` | `()` |
| `get_rule_providers` | GET | `/providers/rules` | `RuleProvidersResponse` |
| `update_rule_provider` | PUT | `/providers/rules/:name` | `()` |
| `get_connections` | GET | `/connections` | `ConnectionsResponse` |
| `close_all_connections` | DELETE | `/connections` | `()` |
| `close_connection` | DELETE | `/connections/:id` | `()` |
| `dns_query` | GET | `/dns/query` | `DnsQueryResponse` |
| `flush_fakeip_cache` | POST | `/cache/fakeip/flush` | `()` |
| `flush_dns_cache` | POST | `/cache/dns/flush` | `()` |
| `restart_core` | POST | `/restart` | `StatusResponse` |
| `upgrade_core` | POST | `/upgrade` | `StatusResponse` |
| `upgrade_ui` | POST | `/upgrade/ui` | `StatusResponse` |
| `upgrade_geo` | POST | `/upgrade/geo` | `()` |
| `debug_gc` | PUT | `/debug/gc` | `()` |
| `stream_traffic` | GET | `/traffic` | `PipeStream<TrafficEntry>` |
| `stream_memory` | GET | `/memory` | `PipeStream<MemoryEntry>` |
| `stream_logs` | GET | `/logs` | `PipeStream<LogEntry>` |
| `stream_logs_structured` | GET | `/logs?format=structured` | `PipeStream<LogStructured>` |
| `stream_connections` | GET | `/connections` | `PipeStream<ConnectionsResponse>` |

---

## 相关文档

- [数据模型](./models.md) — 所有返回类型的字段详解
- [流式读取](./streaming.md) — `PipeStream<T>` 的消费方式和高级用法
- [传输层](./transport.md) — `PipeTransport` 的底层 HTTP 方法
- [错误处理](./error-handling.md) — `ProcessError` 各变体的含义