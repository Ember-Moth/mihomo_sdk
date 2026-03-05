# 传输层

`PipeTransport` 是 SDK 与 mihomo 进程通信的底层传输层，通过 Windows Named Pipe
构建 HTTP/1.1 请求并收发响应。

---

## 目录

- [概述](#概述)
- [创建与配置](#创建与配置)
  - [`PipeTransport::new`](#pipetransportnew)
  - [`with_pipe_name`](#with_pipe_name)
  - [`with_timeout`](#with_timeout)
  - [`with_secret`](#with_secret)
  - [`pipe_name`](#pipe_name)
- [HTTP 方法](#http-方法)
  - [`get`](#get)
  - [`put`](#put)
  - [`post`](#post)
  - [`patch`](#patch)
  - [`delete`](#delete)
- [流式请求](#流式请求)
  - [`stream_get`](#stream_get)
- [`HttpResponse` 结构体](#httpresponse-结构体)
- [连接模型](#连接模型)
- [关于 Named Pipe 认证](#关于-named-pipe-认证)
- [错误处理](#错误处理)
- [直接使用 PipeTransport](#直接使用-pipetransport)
- [内部实现细节](#内部实现细节)

---

## 概述

mihomo 通过配置项 `external-controller-pipe` 暴露一个 Windows Named Pipe，
客户端连接后按标准 HTTP/1.1 协议收发请求/响应。

`PipeTransport` 封装了这一通信过程：

```text
┌──────────────┐         Named Pipe          ┌──────────────┐
│ PipeTransport│  ──── HTTP/1.1 Request ───→  │   mihomo     │
│  (客户端)    │  ←─── HTTP/1.1 Response ───  │  (服务端)    │
└──────────────┘     \\.\pipe\mihomo          └──────────────┘
```

**关键特性**：
- **短连接模型**：每次请求建立一个新的 pipe 连接，发送请求、读取响应后关闭
- **自动重试**：连接 pipe 失败时自动重试（最多 3 次，间隔 50ms）
- **超时保护**：每个请求有独立超时（默认 10 秒）
- **可选认证**：支持 `Authorization: Bearer <secret>` 头
- **流式支持**：`stream_get` 方法返回持续读取的 `PipeStream<T>`

---

## 创建与配置

`PipeTransport` 采用 Builder 模式配置，所有 `with_*` 方法返回 `Self` 支持链式调用。

### `PipeTransport::new`

```rust
pub fn new() -> Self
```

使用默认配置创建传输层。也实现了 `Default` trait。

| 默认值 | 值 |
|--------|----|
| Pipe 名称 | `\\.\pipe\mihomo` |
| 请求超时 | 10 秒 |
| Secret | 无 |

```rust
let transport = PipeTransport::new();

// 等价于：
let transport = PipeTransport::default();
```

---

### `with_pipe_name`

```rust
pub fn with_pipe_name(mut self, name: impl Into<String>) -> Self
```

指定自定义的 Named Pipe 名称。必须与 mihomo 配置中的 `external-controller-pipe`
或 `-ext-ctl-pipe` 命令行参数保持一致。

| 参数 | 类型 | 说明 |
|------|------|------|
| `name` | `impl Into<String>` | pipe 地址，必须以 `\\.\pipe\` 开头 |

```rust
let transport = PipeTransport::new()
    .with_pipe_name(r"\\.\pipe\my_mihomo_instance");
```

---

### `with_timeout`

```rust
pub fn with_timeout(mut self, dur: Duration) -> Self
```

设置**普通请求**（非流式）的超时时间。超时后返回 `ProcessError::Io`（`TimedOut`）。

| 参数 | 类型 | 说明 |
|------|------|------|
| `dur` | `Duration` | 超时时长 |

> 📌 此超时仅影响 `get` / `put` / `post` / `patch` / `delete` 等短连接方法。
> 流式请求 `stream_get` 不受此超时限制（流的生命周期由调用方控制）。

```rust
use std::time::Duration;

let transport = PipeTransport::new()
    .with_timeout(Duration::from_secs(30));
```

---

### `with_secret`

```rust
pub fn with_secret(mut self, secret: impl Into<String>) -> Self
```

设置 API 密钥。设置后，所有请求会自动附加 `Authorization: Bearer <secret>` 头。

| 参数 | 类型 | 说明 |
|------|------|------|
| `secret` | `impl Into<String>` | API 密钥字符串 |

> 📌 根据 mihomo 源码，Named Pipe 通道**不校验 secret**（见 [关于 Named Pipe 认证](#关于-named-pipe-认证)）。
> 设置 secret 不会有副作用，但对 pipe 通道也没有实际效果。

```rust
let transport = PipeTransport::new()
    .with_secret("my_api_key_123");
```

---

### `pipe_name`

```rust
pub fn pipe_name(&self) -> &str
```

获取当前配置的 pipe 名称（只读）。

```rust
let transport = PipeTransport::new()
    .with_pipe_name(r"\\.\pipe\custom");

assert_eq!(transport.pipe_name(), r"\\.\pipe\custom");
```

---

## HTTP 方法

以下五个方法对应 HTTP 的五种常用动词。每次调用都会：

1. 构建完整的 HTTP/1.1 请求报文
2. 连接到 Named Pipe（带自动重试）
3. 发送请求
4. 读取完整响应（直到 EOF）
5. 用 `httparse` 解析响应头和 body
6. 返回 `HttpResponse`

所有方法都是**异步**的，返回 `Result<HttpResponse, ProcessError>`。

---

### `get`

```rust
pub async fn get(&self, path: &str) -> Result<HttpResponse, ProcessError>
```

发送 `GET` 请求。`Content-Length: 0`，无请求体。

| 参数 | 说明 |
|------|------|
| `path` | 请求路径，如 `"/version"`、`"/proxies"` |

```rust
let resp = transport.get("/version").await?;
println!("status={}, body={}", resp.status, resp.body);
```

---

### `put`

```rust
pub async fn put(&self, path: &str, body: &str) -> Result<HttpResponse, ProcessError>
```

发送 `PUT` 请求，附带 JSON 请求体。自动设置 `Content-Type: application/json`
和 `Content-Length`。

| 参数 | 说明 |
|------|------|
| `path` | 请求路径 |
| `body` | JSON 字符串形式的请求体 |

```rust
let body = r#"{"path":"","payload":""}"#;
let resp = transport.put("/configs?force=true", body).await?;
```

---

### `post`

```rust
pub async fn post(&self, path: &str, body: Option<&str>) -> Result<HttpResponse, ProcessError>
```

发送 `POST` 请求，可选 JSON 请求体。

| 参数 | 说明 |
|------|------|
| `path` | 请求路径 |
| `body` | `Some("json")` 附带请求体；`None` 无请求体 |

```rust
// 无请求体
let resp = transport.post("/restart", None).await?;

// 有请求体
let resp = transport.post("/configs/geo", Some("{}")).await?;
```

---

### `patch`

```rust
pub async fn patch(&self, path: &str, body: &str) -> Result<HttpResponse, ProcessError>
```

发送 `PATCH` 请求，附带 JSON 请求体。

| 参数 | 说明 |
|------|------|
| `path` | 请求路径 |
| `body` | JSON 字符串形式的请求体 |

```rust
let resp = transport.patch("/configs", r#"{"mixed-port":7890}"#).await?;
```

---

### `delete`

```rust
pub async fn delete(&self, path: &str) -> Result<HttpResponse, ProcessError>
```

发送 `DELETE` 请求。无请求体。

| 参数 | 说明 |
|------|------|
| `path` | 请求路径 |

```rust
// 关闭所有连接
let resp = transport.delete("/connections").await?;

// 关闭指定连接
let resp = transport.delete("/connections/abc123").await?;
```

---

## 流式请求

### `stream_get`

```rust
pub async fn stream_get<T: DeserializeOwned>(
    &self,
    path: &str,
) -> Result<PipeStream<T>, ProcessError>
```

发送 `GET` 请求并返回一个 [`PipeStream<T>`](./streaming.md)，用于持续接收
换行分隔的 JSON 数据。

| 参数 | 说明 |
|------|------|
| `T` | 每行 JSON 反序列化的目标类型（需实现 `DeserializeOwned`） |
| `path` | 请求路径，如 `"/traffic"`、`"/logs?level=info"` |

**与普通 `get` 的区别**：

| 特性 | `get` | `stream_get` |
|------|-------|--------------|
| 返回值 | `HttpResponse`（完整响应） | `PipeStream<T>`（异步 Stream） |
| 连接生命周期 | 请求完成后关闭 | 持续保持，直到 drop 或服务端关闭 |
| 超时 | 受 `with_timeout` 控制 | 不受超时限制 |
| 响应解析 | 一次性解析 | 逐行解析 |

**适用端点**：

| 端点 | 类型参数 `T` | 说明 |
|------|-------------|------|
| `/traffic` | `TrafficEntry` | 每秒一条流量数据 |
| `/memory` | `MemoryEntry` | 每秒一条内存数据 |
| `/logs` | `LogEntry` | 日志事件（默认格式） |
| `/logs?format=structured` | `LogStructured` | 日志事件（结构化格式） |
| `/connections` | `ConnectionsResponse` | 连接快照 |

**示例**：

```rust
use mihomo_sdk::api::models::TrafficEntry;

let stream = transport.stream_get::<TrafficEntry>("/traffic").await?;
// stream 实现了 futures_core::Stream<Item = Result<TrafficEntry, ProcessError>>
// 见 streaming.md 了解如何消费
```

> 📌 **取消订阅**：drop `PipeStream` 即关闭底层 pipe 连接，是最干净的取消方式。

详细文档见 [流式读取](./streaming.md)。

---

## `HttpResponse` 结构体

```rust
#[derive(Debug)]
pub struct HttpResponse {
    /// HTTP 状态码，例如 200、204、404
    pub status: u16,
    /// 响应体（JSON 字符串或空字符串）
    pub body: String,
}
```

`HttpResponse` 是 `get` / `put` / `post` / `patch` / `delete` 方法的返回值。

| 字段 | 类型 | 说明 |
|------|------|------|
| `status` | `u16` | HTTP 状态码 |
| `body` | `String` | 响应体的原始字符串，通常是 JSON |

**常见状态码**：

| 状态码 | 含义 | 典型场景 |
|--------|------|----------|
| 200 | 成功 | GET 请求返回数据 |
| 204 | 成功（无内容） | PUT / PATCH / DELETE 成功 |
| 400 | 请求错误 | 参数格式不对、代理不可选 |
| 404 | 未找到 | 代理/策略组/Provider 名不存在 |
| 500 | 服务器错误 | 内部异常 |

**使用示例**：

```rust
let resp = transport.get("/version").await?;

if resp.status == 200 {
    let version: VersionResponse = serde_json::from_str(&resp.body)?;
    println!("version = {}", version.version);
} else {
    let err: ApiError = serde_json::from_str(&resp.body)?;
    eprintln!("API error: {}", err.message);
}
```

> 📌 通常你不需要手动检查 `HttpResponse` 的 `status`。
> `MihomoManager` 上的高层 API 方法会自动反序列化 body 并在格式不对时报错。
> 只有在直接使用 `transport.get()` 等低层方法时才需要自己处理。

---

## 连接模型

`PipeTransport` 采用**短连接**模型（每次请求一个连接）：

```text
Request 1:  connect → write request → read response → close
Request 2:  connect → write request → read response → close
Request 3:  connect → write request → read response → close
```

**为什么不使用长连接？**

1. mihomo 的 pipe 端点发送 `Connection: close` 头，服务端在发完响应后关闭连接
2. Named Pipe 上的 HTTP 实现不支持 HTTP/1.1 keep-alive
3. 短连接模型简单可靠，且 pipe 连接的建立开销极小（微秒级）

**流式请求的连接模型**不同：

```text
stream_get:  connect → write request → [持续读取 JSON 行...] → 直到 drop/EOF
```

---

## 关于 Named Pipe 认证

根据 mihomo 源码分析：

**文件**：`hub/route/server.go` — `startPipe` 函数

```go
func startPipe(signal chan struct{}, addr string) {
    // ...
    router := newRouter(/* secret = "" */)  // ← 空 secret
    // ...
}
```

**结论**：Named Pipe 通道的 HTTP 路由器被初始化时 `secret` 参数为空字符串，
这意味着服务端**不校验** `Authorization` 头中的 Bearer token。

**对比**：TCP/TLS 控制器通道（`external-controller`）使用实际的 `secret` 值初始化，
会校验 token。

**实际影响**：

| 通道 | 认证 | 安全性 |
|------|------|--------|
| Named Pipe (`external-controller-pipe`) | 不校验 | 依赖 Windows 管道 ACL |
| TCP (`external-controller`) | 校验 Bearer token | 依赖 secret + 网络隔离 |
| TLS (`external-controller-tls`) | 校验 Bearer token + TLS | 最高 |

因此，即使在 `PipeTransport` 上设置了 `with_secret()`，实际通过 pipe 通道通信时
不会被服务端拒绝——但也不会被校验。

---

## 错误处理

`PipeTransport` 的所有方法返回 `Result<_, ProcessError>`。
可能出现的错误场景：

| 错误 | `ProcessError` 变体 | 原因 |
|------|---------------------|------|
| Pipe 不存在 | `Io`（`NotFound`） | mihomo 未启动，或 pipe 名称不匹配 |
| 连接被拒 | `Io`（`ConnectionRefused`） | 3 次重试后仍无法连接 |
| 请求超时 | `Io`（`TimedOut`） | 请求在超时时间内未完成 |
| 连接中断 | `Io`（`BrokenPipe`） | 服务端意外关闭连接 |
| 响应解析失败 | `Io`（`InvalidData`） | HTTP 响应格式异常 |

**错误处理示例**：

```rust
use mihomo_sdk::ProcessError;

match transport.get("/version").await {
    Ok(resp) => println!("OK: {}", resp.body),
    Err(ProcessError::Io(e)) => {
        match e.kind() {
            std::io::ErrorKind::NotFound => eprintln!("Pipe 不存在，mihomo 是否已启动？"),
            std::io::ErrorKind::TimedOut => eprintln!("请求超时"),
            std::io::ErrorKind::BrokenPipe => eprintln!("连接中断"),
            _ => eprintln!("IO 错误: {}", e),
        }
    }
    Err(e) => eprintln!("其他错误: {}", e),
}
```

---

## 直接使用 PipeTransport

通常你通过 `MihomoManager` 间接使用 `PipeTransport`。但以下场景你可能需要直接使用：

### 场景 1：连接已运行的 mihomo（不管理进程）

```rust
use mihomo_sdk::PipeTransport;

let transport = PipeTransport::new()
    .with_pipe_name(r"\\.\pipe\mihomo");

// 直接调用 API，无需 MihomoManager
let resp = transport.get("/version").await?;
println!("{}", resp.body);

let resp = transport.get("/configs").await?;
println!("{}", resp.body);
```

### 场景 2：调用 SDK 未封装的端点

```rust
// 通过 MihomoManager 获取 transport 引用
let resp = mgr.api().get("/some/future/endpoint").await?;

// 或直接使用 transport 对象
let resp = transport.post("/some/action", Some(r#"{"key":"value"}"#)).await?;
```

### 场景 3：自定义请求处理逻辑

```rust
let resp = transport.get("/proxies").await?;

if resp.status == 200 {
    // 自定义 JSON 解析逻辑（而非使用 SDK 提供的模型）
    let value: serde_json::Value = serde_json::from_str(&resp.body)?;
    let proxy_count = value["proxies"].as_object().map_or(0, |m| m.len());
    println!("共 {} 个代理", proxy_count);
} else {
    eprintln!("请求失败: HTTP {}", resp.status);
}
```

---

## 内部实现细节

以下信息面向想要深入了解或贡献代码的开发者。

### 请求报文格式

`build_request` 方法构建的 HTTP 报文格式：

```http
GET /version HTTP/1.1
Host: mihomo
Connection: close
Authorization: Bearer <secret>     ← 仅当设置了 secret 时
Content-Length: 0                   ← 无 body 时
Content-Type: application/json      ← 有 body 时
Content-Length: <body_len>          ← 有 body 时

<body>                              ← 有 body 时
```

### 连接重试策略

| 参数 | 值 |
|------|----|
| 最大重试次数 | 3 |
| 重试间隔 | 50ms |
| 使用的 API | `tokio::net::windows::named_pipe::ClientOptions::new().open()` |

重试仅针对**连接阶段**。一旦连接成功，写入或读取阶段的错误会直接返回。

### 响应解析

使用 `httparse` crate 解析 HTTP 响应：

1. `httparse::Response::parse()` 解析状态行和响应头
2. 提取 `status_code`
3. 头部之后的所有字节作为 `body`
4. body 使用 `String::from_utf8_lossy` 转换

> 📌 当前实现不处理 `Transfer-Encoding: chunked`。
> mihomo 在 pipe 通道上不使用 chunked 编码（短连接请求直接关闭连接），
> 但流式端点的分块由 `PipeStream` 按行处理。

### 线程安全

`PipeTransport` 实现了 `Clone`、`Debug` 和 `Send + Sync`（通过 derive）。

- **无共享状态**：所有字段（`pipe_name`、`timeout`、`secret`）在构造后不可变
- **每请求新连接**：不持有长期连接，无需内部锁
- **可安全跨任务共享**：多个 tokio 任务可以同时使用同一个 `PipeTransport` 实例

---

## 相关文档

- [流式读取](./streaming.md) — `PipeStream<T>` 的详细使用文档
- [进程管理](./process-management.md) — `MihomoManager` 如何使用 `PipeTransport`
- [REST API 参考](./api-reference.md) — 通过 `MihomoManager` 封装的高层 API
- [错误处理](./error-handling.md) — `ProcessError` 各变体的完整说明