# 流式读取

`PipeStream<T>` 是一个异步 `Stream` 适配器，用于从 Windows Named Pipe 连接中
持续读取换行分隔的 JSON 对象。适用于 mihomo 的实时推送端点。

---

## 目录

- [概述](#概述)
- [支持的流式端点](#支持的流式端点)
- [PipeStream\<T\> 结构体](#pipestreamlttgt-结构体)
  - [生命周期与状态机](#生命周期与状态机)
  - [`http_status`](#http_status)
  - [Stream trait 实现](#stream-trait-实现)
  - [取消（Cancellation）](#取消cancellation)
- [高层 API 方法](#高层-api-方法)
  - [`stream_traffic`](#stream_traffic)
  - [`stream_memory`](#stream_memory)
  - [`stream_logs`](#stream_logs)
  - [`stream_logs_structured`](#stream_logs_structured)
  - [`stream_connections`](#stream_connections)
- [消费 Stream 的方式](#消费-stream-的方式)
  - [方式 1：手动 poll_fn（零额外依赖）](#方式-1手动-poll_fn零额外依赖)
  - [方式 2：tokio-stream StreamExt](#方式-2tokio-stream-streamext)
  - [方式 3：futures StreamExt](#方式-3futures-streamext)
  - [方式 4：Box::pin（无需 tokio::pin!）](#方式-4boxpin无需-tokiopin)
- [典型使用模式](#典型使用模式)
  - [限时采集 N 条数据](#限时采集-n-条数据)
  - [后台监控任务](#后台监控任务)
  - [多流并发订阅](#多流并发订阅)
  - [带超时的逐条读取](#带超时的逐条读取)
- [与非流式 API 的对比](#与非流式-api-的对比)
- [关于 /connections 端点](#关于-connections-端点)
- [错误处理](#错误处理)
- [内部实现细节](#内部实现细节)
  - [状态机详解](#状态机详解)
  - [HTTP 头解析](#http-头解析)
  - [行解析（drain_lines）](#行解析drain_lines)
  - [EOF 处理](#eof-处理)
  - [唤醒策略](#唤醒策略)
- [性能与资源](#性能与资源)

---

## 概述

mihomo 的部分 REST API 端点采用**流式响应**模式：服务端保持 HTTP 连接打开，
持续输出换行分隔（`\n`-delimited）的 JSON 对象。每个 JSON 对象是一行，
客户端持续读取即可获得实时推送的数据。

```text
HTTP/1.1 200 OK\r\n
Content-Type: application/json\r\n
\r\n
{"up":0,"down":0}\n
{"up":1024,"down":2048}\n
{"up":512,"down":1024}\n
...（持续输出直到连接关闭）
```

`PipeStream<T>` 封装了这个过程：

1. 自动解析并剥离 HTTP 响应头
2. 按 `\n` 分行，每行反序列化为类型 `T`
3. 实现标准的 `futures_core::Stream<Item = Result<T, ProcessError>>` trait
4. Drop 时自动关闭底层管道连接

---

## 支持的流式端点

| 端点 | 推送频率 | 数据类型 `T` | 高层方法 | 说明 |
|------|----------|-------------|----------|------|
| `GET /traffic` | ~1秒 | `TrafficEntry` | `stream_traffic()` | 实时上下行速率和累计总量 |
| `GET /memory` | ~1秒 | `MemoryEntry` | `stream_memory()` | 堆内存使用量和 OS 限制 |
| `GET /logs` | 事件驱动 | `LogEntry` | `stream_logs(level)` | 日志（默认格式） |
| `GET /logs?format=structured` | 事件驱动 | `LogStructured` | `stream_logs_structured(level)` | 日志（结构化格式） |
| `GET /connections` | 事件驱动 | `ConnectionsResponse` | `stream_connections()` | 连接快照 |

---

## PipeStream\<T\> 结构体

```rust
pub struct PipeStream<T> {
    pipe: NamedPipeClient,   // #[pin] — 底层管道连接
    buf: Vec<u8>,            // 读取缓冲区
    pending: Vec<T>,         // 已解析但尚未 yield 的队列
    state: StreamState,      // 当前生命周期状态
    http_status: u16,        // HTTP 状态码（头解析后填充）
    _marker: PhantomData<T>,
}
```

`PipeStream<T>` 不可直接构造——通过以下方式获取：

- `PipeTransport::stream_get::<T>(path)` — 底层方法
- `MihomoManager::stream_traffic()` — 高层便捷方法
- `MihomoManager::stream_memory()` 等

### 生命周期与状态机

```text
  ┌─────────────────┐
  │  ReadingHeader   │  ← 初始状态：读取并消费 HTTP 响应头
  └────────┬────────┘
           │ 找到 \r\n\r\n，解析成功
           ▼
  ┌─────────────────┐
  │  ReadingBody     │  ← 核心状态：逐行读取 JSON 并 yield
  └────────┬────────┘
           │ EOF / BrokenPipe / 非 2xx 状态码
           ▼
  ┌─────────────────┐
  │     Done         │  ← 终态：Stream 返回 None
  └─────────────────┘
```

### `http_status`

```rust
pub fn http_status(&self) -> u16
```

返回 HTTP 响应头中的状态码。

| 返回值 | 含义 |
|--------|------|
| `0` | 头部尚未完全接收和解析 |
| `200` | 正常，流式数据即将/已经到来 |
| `≥300` | 服务端返回错误，Stream 的第一个 item 将是 `Err` |

> 📌 通常不需要手动检查此值。如果服务端返回非 2xx 状态码，`poll_next` 会自动
> 产出一个包含错误信息的 `Err` 并结束流。

### Stream trait 实现

```rust
impl<T: DeserializeOwned> Stream for PipeStream<T> {
    type Item = Result<T, ProcessError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>>;
}
```

- `Poll::Ready(Some(Ok(item)))` — 成功读取并反序列化一条数据
- `Poll::Ready(Some(Err(e)))` — 遇到错误（HTTP 非 2xx / IO 错误）
- `Poll::Ready(None)` — 流结束（EOF / BrokenPipe）
- `Poll::Pending` — 数据尚未到达，等待更多数据

### 取消（Cancellation）

**丢弃 `PipeStream` 即可取消订阅**。这会关闭底层的 `NamedPipeClient` 句柄，
干净地断开与 mihomo 的连接。

```rust
{
    let stream = mgr.stream_traffic().await?;
    // stream 在这里被 drop
}
// 管道连接已关闭，mihomo 侧对应的 goroutine 也会退出
```

> 📌 取消是**即时的**、**非阻塞的**，不需要 await 任何清理操作。

---

## 高层 API 方法

以下方法定义在 `MihomoManager` 上，是对 `PipeTransport::stream_get` 的类型化封装。

### `stream_traffic`

```rust
pub async fn stream_traffic(&self) -> Result<PipeStream<TrafficEntry>, ProcessError>
```

**端点**：`GET /traffic`

**来源**：`hub/route/server.go` — `traffic` handler

**推送频率**：约每 1 秒

**数据结构**（`TrafficEntry`）：

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `up` | `i64` | `up` | 瞬时上行速率（字节/秒） |
| `down` | `i64` | `down` | 瞬时下行速率（字节/秒） |
| `up_total` | `i64` | `upTotal` | 累计上行总量（字节） |
| `down_total` | `i64` | `downTotal` | 累计下行总量（字节） |

> 📌 `up_total` 和 `down_total` 在旧版 mihomo 中可能不存在，模型中设为 `#[serde(default)]`，
> 缺失时默认为 `0`。

**示例**：

```rust
use futures_core::Stream;
use std::pin::pin;
use std::future::poll_fn;
use std::task::Poll;

let stream = mgr.stream_traffic().await?;
let mut pinned = pin!(stream);

for _ in 0..5 {
    let entry = poll_fn(|cx| match pinned.as_mut().poll_next(cx) {
        Poll::Ready(Some(Ok(e))) => Poll::Ready(e),
        Poll::Ready(Some(Err(e))) => panic!("error: {}", e),
        Poll::Ready(None) => panic!("stream ended"),
        Poll::Pending => Poll::Pending,
    }).await;
    println!("↑ {} B/s  ↓ {} B/s", entry.up, entry.down);
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

**数据结构**（`MemoryEntry`）：

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `inuse` | `u64` | `inuse` | 当前 Go 运行时堆内存使用量（字节） |
| `oslimit` | `u64` | `oslimit` | 操作系统内存限制（字节），0 表示无限制 |

> 📌 第一条推送的 `inuse` 可能为 `0`（Go 运行时尚未更新统计），后续条目会反映真实值。

**示例**：

```rust
let stream = mgr.stream_memory().await?;
let mut pinned = pin!(stream);

// 读取 3 条
for _ in 0..3 {
    if let Poll::Ready(Some(Ok(entry))) = poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
        let mb = entry.inuse as f64 / 1024.0 / 1024.0;
        println!("内存使用: {:.2} MB", mb);
    }
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

**端点**：`GET /logs` 或 `GET /logs?level=<level>`

**来源**：`hub/route/server.go` — `getLogs` handler

**推送频率**：事件驱动（有日志时才推送）

| 参数 | 类型 | 说明 |
|------|------|------|
| `level` | `&str` | 最低日志级别过滤：`"debug"` / `"info"` / `"warning"` / `"error"` / `"silent"`。传空字符串 `""` 不过滤。 |

**数据结构**（`LogEntry`）：

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `level` | `String` | `type` | 日志级别：`"info"` / `"warning"` / `"error"` / `"debug"` |
| `payload` | `String` | `payload` | 日志正文 |

> ⚠️ **注意**：JSON 中的键名是 `type`（不是 `level`），模型中使用 `#[serde(rename = "type")]` 映射。

> 📌 **关于日志产生**：如果 mihomo 处于空闲状态（无流量、无配置变更），可能长时间不产生日志。
> 可以通过 `patch_configs` 修改配置来触发日志输出（例如切换端口号）。

**示例**：

```rust
// 只订阅 info 及以上级别的日志
let stream = mgr.stream_logs("info").await?;
let mut pinned = pin!(stream);

loop {
    match poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
        Some(Ok(entry)) => println!("[{}] {}", entry.level, entry.payload),
        Some(Err(e)) => { eprintln!("日志流错误: {}", e); break; }
        None => { println!("日志流结束"); break; }
    }
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

**端点**：`GET /logs?format=structured` 或 `GET /logs?format=structured&level=<level>`

**来源**：`hub/route/server.go` — `getLogs` handler（`format=structured` 分支）

| 参数 | 类型 | 说明 |
|------|------|------|
| `level` | `&str` | 最低日志级别过滤，与 `stream_logs` 相同 |

**数据结构**（`LogStructured`）：

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `time` | `String` | `time` | 日志时间戳（ISO 8601 格式） |
| `level` | `String` | `level` | 日志级别 |
| `message` | `String` | `message` | 日志消息正文 |
| `fields` | `Vec<LogStructuredField>` | `fields` | 附加的键值对数组 |

**`LogStructuredField`**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `key` | `String` | 键名 |
| `value` | `String` | 值 |

**示例**：

```rust
let stream = mgr.stream_logs_structured("debug").await?;
let mut pinned = pin!(stream);

// 读取 5 条结构化日志
for _ in 0..5 {
    if let Some(Ok(entry)) = poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
        print!("[{}] {} {}", entry.time, entry.level, entry.message);
        for field in &entry.fields {
            print!(" {}={}", field.key, field.value);
        }
        println!();
    }
}
```

---

### `stream_connections`

```rust
pub async fn stream_connections(
    &self,
) -> Result<PipeStream<ConnectionsResponse>, ProcessError>
```

**端点**：`GET /connections`

**来源**：`hub/route/connections.go` — `getConnections`（流式路径）

**数据结构**（`ConnectionsResponse`）：

| 字段 | 类型 | JSON 键名 | 说明 |
|------|------|-----------|------|
| `download_total` | `u64` | `downloadTotal` | 累计下行总量 |
| `upload_total` | `u64` | `uploadTotal` | 累计上行总量 |
| `connections` | `Option<Vec<ConnectionInfo>>` | `connections` | 活跃连接列表（可能为 `null`） |
| `memory` | `u64` | `memory` | 内存使用量 |

> ⚠️ **重要限制**：mihomo 的 `/connections` 端点在 Named Pipe 上使用**非 WebSocket** 模式。
> 服务端通常返回一次快照后关闭连接，而非持续推送。
> 因此 `stream_connections()` 可能只产生 1 条数据后流结束。
> 这是 mihomo 的行为，非 SDK 缺陷。
>
> 如需获取连接快照，推荐使用非流式方法 `get_connections()`。
> 详见 [关于 /connections 端点](#关于-connections-端点)。

**示例**：

```rust
let stream = mgr.stream_connections().await?;
let mut stream = Box::pin(stream);

// 收集所有产出的快照（通常只有 1 条）
let mut snapshots = Vec::new();
while let Some(Ok(snap)) = poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
    snapshots.push(snap);
}
for (i, snap) in snapshots.iter().enumerate() {
    let count = snap.connections.as_ref().map_or(0, |c| c.len());
    println!("快照#{}: {} 个活跃连接, ↓{} ↑{}", i, count, snap.download_total, snap.upload_total);
}
```

---

## 消费 Stream 的方式

`PipeStream<T>` 实现了标准的 `futures_core::Stream` trait。
以下是几种消费方式，按依赖从少到多排列。

### 方式 1：手动 poll_fn（零额外依赖）

不需要任何额外 crate，仅使用标准库和 `futures_core`：

```rust
use futures_core::Stream;
use std::future::poll_fn;
use std::pin::pin;
use std::task::Poll;

let stream = mgr.stream_traffic().await?;
let mut pinned = pin!(stream);

loop {
    let item = poll_fn(|cx| match pinned.as_mut().poll_next(cx) {
        Poll::Ready(item) => Poll::Ready(item),
        Poll::Pending => Poll::Pending,
    }).await;

    match item {
        Some(Ok(entry)) => println!("up={} down={}", entry.up, entry.down),
        Some(Err(e)) => { eprintln!("错误: {}", e); break; }
        None => { println!("流结束"); break; }
    }
}
```

### 方式 2：tokio-stream StreamExt

添加依赖 `tokio-stream = "0.1"`：

```rust
use tokio_stream::StreamExt;

let stream = mgr.stream_traffic().await?;
tokio::pin!(stream);

while let Some(result) = stream.next().await {
    match result {
        Ok(entry) => println!("up={} down={}", entry.up, entry.down),
        Err(e) => { eprintln!("错误: {}", e); break; }
    }
}
```

### 方式 3：futures StreamExt

添加依赖 `futures = "0.3"`：

```rust
use futures::StreamExt;

let stream = mgr.stream_traffic().await?;
tokio::pin!(stream);

while let Some(result) = stream.next().await {
    match result {
        Ok(entry) => println!("up={} down={}", entry.up, entry.down),
        Err(e) => { eprintln!("错误: {}", e); break; }
    }
}
```

### 方式 4：Box::pin（无需 tokio::pin!）

如果你不想使用 `pin!` 宏（例如在非 `async fn` 上下文中需要将流存入结构体），
可以使用 `Box::pin`：

```rust
use futures_core::Stream;
use std::pin::Pin;

let stream = mgr.stream_traffic().await?;
let mut stream: Pin<Box<_>> = Box::pin(stream);

// stream 现在是 Pin<Box<PipeStream<TrafficEntry>>>
// 可以存入结构体、跨 await 使用等
```

---

## 典型使用模式

### 限时采集 N 条数据

```rust
use tokio::time::{timeout, Duration};
use futures_core::Stream;
use std::pin::pin;
use std::future::poll_fn;
use std::task::Poll;

async fn collect_traffic(
    mgr: &MihomoManager,
    count: usize,
    max_wait: Duration,
) -> Result<Vec<TrafficEntry>, Box<dyn std::error::Error>> {
    let stream = mgr.stream_traffic().await?;
    let mut pinned = pin!(stream);
    let mut items = Vec::with_capacity(count);

    let result = timeout(max_wait, async {
        while items.len() < count {
            match poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
                Some(Ok(entry)) => items.push(entry),
                Some(Err(e)) => return Err(e.into()),
                None => break,
            }
        }
        Ok(())
    }).await;

    match result {
        Ok(Ok(())) => Ok(items),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(items), // 超时，返回已采集的数据
    }
}
```

### 后台监控任务

```rust
use tokio::sync::watch;
use futures_core::Stream;
use std::pin::pin;
use std::future::poll_fn;
use std::task::Poll;

async fn start_traffic_monitor(
    mgr: MihomoManager,
    tx: watch::Sender<Option<TrafficEntry>>,
) {
    loop {
        // 如果连接断开，尝试重连
        let stream = match mgr.stream_traffic().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("连接失败: {}, 3 秒后重试", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };

        let mut pinned = pin!(stream);
        loop {
            match poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
                Some(Ok(entry)) => {
                    let _ = tx.send(Some(entry));
                }
                Some(Err(e)) => {
                    eprintln!("流错误: {}, 重连中...", e);
                    break; // 外层循环会重连
                }
                None => {
                    eprintln!("流结束，重连中...");
                    break;
                }
            }
        }
    }
}

// 使用方式：
// let (tx, rx) = watch::channel(None);
// tokio::spawn(start_traffic_monitor(mgr.clone(), tx));
// 在其他地方通过 rx.borrow() 读取最新的流量数据
```

### 多流并发订阅

```rust
use futures_core::Stream;
use std::pin::pin;
use std::future::poll_fn;
use std::task::Poll;

async fn monitor_all(mgr: &MihomoManager) -> Result<(), Box<dyn std::error::Error>> {
    // 同时打开多个流
    let traffic_stream = mgr.stream_traffic().await?;
    let memory_stream = mgr.stream_memory().await?;
    let log_stream = mgr.stream_logs("info").await?;

    // 各自在独立的 task 中消费
    let mgr_t = mgr.clone();
    let t1 = tokio::spawn(async move {
        let mut s = pin!(traffic_stream);
        while let Some(Ok(e)) = poll_fn(|cx| s.as_mut().poll_next(cx)).await {
            println!("[traffic] ↑{} ↓{}", e.up, e.down);
        }
    });

    let t2 = tokio::spawn(async move {
        let mut s = pin!(memory_stream);
        while let Some(Ok(e)) = poll_fn(|cx| s.as_mut().poll_next(cx)).await {
            println!("[memory] {:.2} MB", e.inuse as f64 / 1024.0 / 1024.0);
        }
    });

    let t3 = tokio::spawn(async move {
        let mut s = pin!(log_stream);
        while let Some(Ok(e)) = poll_fn(|cx| s.as_mut().poll_next(cx)).await {
            println!("[log] [{}] {}", e.level, e.payload);
        }
    });

    // 等待任意一个结束
    tokio::select! {
        _ = t1 => println!("traffic 流结束"),
        _ = t2 => println!("memory 流结束"),
        _ = t3 => println!("log 流结束"),
    }

    Ok(())
}
```

### 带超时的逐条读取

```rust
use tokio::time::{timeout, Duration};
use futures_core::Stream;
use std::pin::pin;
use std::future::poll_fn;
use std::task::Poll;

let stream = mgr.stream_traffic().await?;
let mut pinned = pin!(stream);

loop {
    // 每条数据最多等待 5 秒
    let result = timeout(
        Duration::from_secs(5),
        poll_fn(|cx| pinned.as_mut().poll_next(cx)),
    ).await;

    match result {
        Ok(Some(Ok(entry))) => println!("up={} down={}", entry.up, entry.down),
        Ok(Some(Err(e))) => { eprintln!("流错误: {}", e); break; }
        Ok(None) => { println!("流正常结束"); break; }
        Err(_) => { eprintln!("5 秒内未收到数据，可能 mihomo 已停止"); break; }
    }
}
```

---

## 与非流式 API 的对比

| 特性 | 非流式（如 `get_connections`） | 流式（如 `stream_traffic`） |
|------|-------------------------------|---------------------------|
| 返回类型 | `Result<T, ProcessError>` | `Result<PipeStream<T>, ProcessError>` |
| 数据量 | 单次快照 | 持续推送 |
| 连接模型 | 短连接（请求-响应-关闭） | 长连接（持续读取） |
| 超时 | 受 `PipeTransport::with_timeout` 控制 | 无内建超时（由调用方控制） |
| 适用场景 | 一次性查询 | 实时监控 |
| 资源消耗 | 低（瞬时） | 持续占用一个 pipe 连接 |

---

## 关于 /connections 端点

`/connections` 端点在 mihomo 中有两种行为：

| 连接方式 | 行为 |
|----------|------|
| **WebSocket 升级**（TCP/TLS 控制器） | 持续推送连接快照 |
| **普通 HTTP**（Named Pipe） | 返回一次快照后关闭连接 |

由于 Named Pipe 传输层不支持 WebSocket 升级协议，`stream_connections()` 在 pipe
通道上实际等同于"读取一次快照后流结束"。

**推荐做法**：

```rust
// ✅ 一次性获取连接快照
let snapshot = mgr.get_connections().await?;

// ✅ 如果需要定期轮询，使用定时器 + 普通 API
loop {
    let snapshot = mgr.get_connections().await?;
    println!("活跃连接: {}", snapshot.connections.as_ref().map_or(0, |c| c.len()));
    tokio::time::sleep(Duration::from_secs(1)).await;
}

// ⚠️ stream_connections() 在 pipe 上通常只产出 1 条数据
let stream = mgr.stream_connections().await?;
// 流很快结束...
```

---

## 错误处理

`PipeStream` 的每个 item 都是 `Result<T, ProcessError>`。可能产生错误的场景：

### 连接阶段错误（在 stream_get 中发生）

这些错误在创建 `PipeStream` 之前就会触发，由 `stream_get` 的 `Result` 返回：

| 错误 | 原因 |
|------|------|
| `ProcessError::Io`（NotFound） | Pipe 不存在（mihomo 未启动） |
| `ProcessError::Io`（ConnectionRefused） | 3 次重试后仍无法连接 |

### 流式读取阶段错误（由 poll_next 产出）

| 错误 | 原因 | 流是否结束 |
|------|------|-----------|
| HTTP 非 2xx 状态码 | 服务端返回错误（如 404、500） | 是（状态变为 Done） |
| IO 错误（非 BrokenPipe） | 底层管道读取异常 | 是 |
| BrokenPipe | 服务端关闭连接 | 是（正常结束，返回 `None`） |
| EOF | 服务端关闭连接 | 是（正常结束） |

> 📌 **JSON 解析失败不会产生错误**——无法反序列化的行会被静默跳过。
> 这是为了容忍可能的分块传输编码中的十六进制长度行、空行或其他非 JSON 内容。

**错误处理示例**：

```rust
let stream = match mgr.stream_traffic().await {
    Ok(s) => s,
    Err(e) => {
        eprintln!("无法打开流: {}", e);
        return;
    }
};

let mut pinned = pin!(stream);
loop {
    match poll_fn(|cx| pinned.as_mut().poll_next(cx)).await {
        Some(Ok(entry)) => {
            // 正常数据
            println!("up={}", entry.up);
        }
        Some(Err(e)) => {
            // 流级错误，通常意味着流结束
            eprintln!("流错误: {}", e);
            break;
        }
        None => {
            // 正常结束
            println!("流结束");
            break;
        }
    }
}
```

---

## 内部实现细节

以下信息面向想要深入了解实现原理或贡献代码的开发者。

### 状态机详解

`poll_next` 的每次调用按以下流程执行：

```text
1. pending 队列非空？→ 直接 yield 一条，返回 Ready
2. 状态为 Done？→ 返回 Ready(None)
3. 从 pipe 异步读取数据到 buf
   - Pending → 返回 Pending
   - Err(BrokenPipe) → 状态→Done，返回 Ready(None)
   - Err(other) → 状态→Done，返回 Ready(Some(Err))
   - Ok(0 bytes) → EOF
     - buf 中有残余数据？尝试解析为最后一条
     - 状态→Done，返回 Ready(None)
   - Ok(n bytes) → 追加到 buf
4. 状态为 ReadingHeader？
   - buf 中找到 \r\n\r\n？
     - 解析状态码
     - 非 2xx → 返回 Ready(Some(Err))，状态→Done
     - 2xx → 剥离头部，状态→ReadingBody
   - 未找到 → 唤醒 waker，返回 Pending
5. 状态为 ReadingBody
   - 调用 drain_lines：从 buf 中提取完整行并反序列化
   - 有结果？yield 一条
   - 无结果 → 唤醒 waker，返回 Pending
```

### HTTP 头解析

使用两个辅助函数，不依赖 `httparse`（`PipeStream` 使用自己的轻量解析）：

**`find_header_end(buf)`**：在字节缓冲区中搜索 `\r\n\r\n` 序列，返回首个 `\r` 的位置。

**`parse_status_code(header_bytes)`**：从 HTTP 状态行（如 `HTTP/1.1 200 OK`）中提取状态码。
使用简单的空格分割，不需要完整的 HTTP 解析器。

### 行解析（drain_lines）

```rust
fn drain_lines<T: DeserializeOwned>(buf: &mut Vec<u8>, out: &mut Vec<T>)
```

从 `buf` 中提取所有完整的 `\n` 结尾行：

1. 找到第一个 `\n` 的位置
2. 取出该行（不含 `\n`），trim 后尝试 `serde_json::from_str`
3. 解析成功 → push 到 `out`
4. 解析失败 → 静默跳过
5. 从 buf 中移除已处理的字节（含 `\n`）
6. 重复直到没有更多完整行

不完整的行（没有 `\n` 结尾）保留在 `buf` 中，等待下次读取补全。

### EOF 处理

当 pipe 读取返回 0 字节（EOF）时：

1. 检查 buf 中是否有残余数据
2. 如果有，尝试将其作为最后一条 JSON 解析
3. 如果解析成功，yield 该条目
4. 无论是否成功，状态变为 Done

这确保了即使服务端在发送最后一行 JSON 后不附加 `\n` 就关闭连接，
数据也不会丢失。

### 唤醒策略

在两种情况下，`poll_next` 会调用 `cx.waker().wake_by_ref()` 后返回 `Pending`：

1. **ReadingHeader 状态，头部未完全接收**：数据不够，需要更多读取
2. **ReadingBody 状态，无完整行**：数据不够，需要更多读取

这种"自唤醒"策略确保 tokio 运行时会立即再次 poll 此 future，
驱动后续的 pipe 读取。这在语义上等同于"我还没准备好 yield 一个 item，
但请尽快再问我一次"。

> 📌 这不会导致忙等待，因为下一次 poll 时 `pipe.poll_read` 会正确返回 `Pending`
> （如果管道中确实没有新数据），此时 tokio 会将任务挂起直到管道可读。

---

## 性能与资源

| 指标 | 说明 |
|------|------|
| **读取缓冲区** | 8192 字节（每个 PipeStream 独立分配） |
| **内存开销** | 约 8KB + pending 队列中的已解析对象 |
| **Pipe 连接数** | 每个活跃的 PipeStream 占用 1 个 pipe 连接 |
| **CPU 开销** | 极低——仅在有数据到达时才做 JSON 解析 |
| **推荐并发流数** | 无硬性限制，但每个流占一个 pipe slot，建议 ≤10 个 |

**资源释放**：

- Drop `PipeStream` → 关闭 pipe handle → 释放所有内部缓冲区
- mihomo 侧对应的 goroutine 在检测到 pipe 关闭后也会退出

---

## 相关文档

- [传输层](./transport.md) — `PipeTransport::stream_get` 的底层实现
- [数据模型](./models.md) — `TrafficEntry`、`MemoryEntry`、`LogEntry` 等结构体详解
- [REST API 参考](./api-reference.md) — 非流式 API 方法
- [错误处理](./error-handling.md) — `ProcessError` 枚举的完整说明