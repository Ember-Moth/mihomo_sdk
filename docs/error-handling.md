# 错误处理

`ProcessError` 是 `mihomo_sdk` 中所有操作的统一错误类型。
本文档详细说明每个错误变体的含义、触发场景和推荐的处理方式。

---

## 目录

- [ProcessError 枚举](#processerror-枚举)
  - [`BinaryNotFound`](#binarynotfound)
  - [`ConfigNotFound`](#confignotfound)
  - [`AlreadyRunning`](#alreadyrunning)
  - [`NotRunning`](#notrunning)
  - [`Io`](#io)
  - [`NotReady`](#notready)
- [各层错误来源](#各层错误来源)
  - [进程管理层](#进程管理层)
  - [传输层（PipeTransport）](#传输层pipetransport)
  - [API 方法层](#api-方法层)
  - [流式读取层（PipeStream）](#流式读取层pipestream)
- [常见 IO 错误细分](#常见-io-错误细分)
- [错误处理模式](#错误处理模式)
  - [基本 match](#基本-match)
  - [按层分类处理](#按层分类处理)
  - [可恢复 vs 不可恢复](#可恢复-vs-不可恢复)
  - [重试策略](#重试策略)
  - [日志记录](#日志记录)
- [与 mihomo API 错误的关系](#与-mihomo-api-错误的关系)
- [在异步上下文中的使用](#在异步上下文中的使用)
- [自定义错误转换](#自定义错误转换)

---

## ProcessError 枚举

```rust
#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("binary not found: {0}")]
    BinaryNotFound(PathBuf),

    #[error("config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("process already running (pid={0})")]
    AlreadyRunning(u32),

    #[error("process not running")]
    NotRunning,

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("api not ready after {0} attempts")]
    NotReady(u32),
}
```

`ProcessError` 使用 `thiserror` crate 派生 `std::error::Error` 和 `Display` trait，
同时实现了 `From<std::io::Error>` 自动转换。

---

### `BinaryNotFound`

```rust
BinaryNotFound(PathBuf)
```

**含义**：指定的 mihomo 可执行文件路径不存在。

**触发场景**：
- `MihomoManager::start()` 时检测到 `binary_path` 指向的文件不存在

**附带数据**：`PathBuf` — 不存在的二进制文件路径

**处理建议**：

| 方案 | 说明 |
|------|------|
| 检查路径 | 确认 `mihomo.exe` 是否在预期位置 |
| 更新路径 | 调用 `mgr.set_binary_path(correct_path).await` |
| 提示用户 | 在 UI 中显示"请下载或指定 mihomo 可执行文件" |

**示例**：

```rust
match mgr.start().await {
    Err(ProcessError::BinaryNotFound(path)) => {
        eprintln!("找不到 mihomo 可执行文件: {:?}", path);
        eprintln!("请将 mihomo.exe 放到项目目录下");
    }
    // ...
}
```

---

### `ConfigNotFound`

```rust
ConfigNotFound(PathBuf)
```

**含义**：指定的配置文件路径不存在。

**触发场景**：
- `MihomoManager::start()` 时检测到 `config_path` 指向的文件不存在

**附带数据**：`PathBuf` — 不存在的配置文件路径

**处理建议**：

| 方案 | 说明 |
|------|------|
| 检查路径 | 确认配置文件路径是否正确 |
| 创建默认配置 | 自动生成一个最小配置文件 |
| 更新路径 | 调用 `mgr.set_config_path(correct_path).await` |

**示例**：

```rust
match mgr.start().await {
    Err(ProcessError::ConfigNotFound(path)) => {
        eprintln!("配置文件不存在: {:?}", path);
        // 自动创建最小配置
        std::fs::write(&path, "mixed-port: 7890\n")?;
        // 重试
        mgr.start().await?;
    }
    // ...
}
```

---

### `AlreadyRunning`

```rust
AlreadyRunning(u32)
```

**含义**：mihomo 进程已经在运行中，不能重复启动。

**触发场景**：
- 调用 `MihomoManager::start()` 时，内部已持有一个活跃的子进程

**附带数据**：`u32` — 当前正在运行的进程 PID

**处理建议**：

| 方案 | 说明 |
|------|------|
| 忽略 | 已经在运行，通常不需要额外操作 |
| 使用 restart | 调用 `mgr.restart().await` 替代 `start()` |
| 先 stop | 调用 `mgr.stop().await` 后再 `start()` |

**示例**：

```rust
match mgr.start().await {
    Ok(pid) => println!("已启动, PID = {}", pid),
    Err(ProcessError::AlreadyRunning(pid)) => {
        println!("进程已在运行, PID = {}, 无需重复启动", pid);
    }
    Err(e) => return Err(e.into()),
}
```

---

### `NotRunning`

```rust
NotRunning
```

**含义**：尝试操作一个未运行的进程。

**触发场景**：
- 调用 `MihomoManager::stop()` 时，没有活跃的子进程
- 调用 `MihomoManager::wait_ready()` 时，在重试过程中发现进程已退出

**附带数据**：无

**处理建议**：

| 方案 | 说明 |
|------|------|
| 忽略（stop 场景） | 进程已停止，目标达成 |
| 重新启动（wait_ready 场景） | 进程可能崩溃退出，需要重新 `start()` |
| 检查退出原因 | 查看 mihomo 的标准错误输出 |

**示例**：

```rust
// stop 场景 — 通常可以忽略
match mgr.stop().await {
    Ok(()) => println!("已停止"),
    Err(ProcessError::NotRunning) => println!("进程本来就没运行"),
    Err(e) => return Err(e.into()),
}

// wait_ready 场景 — 进程可能崩溃了
match mgr.wait_ready(20, Duration::from_millis(500)).await {
    Ok(()) => println!("API 已就绪"),
    Err(ProcessError::NotRunning) => {
        eprintln!("mihomo 进程在启动过程中退出了！");
        eprintln!("请检查配置文件是否正确");
    }
    Err(e) => return Err(e.into()),
}
```

---

### `Io`

```rust
Io(#[from] io::Error)
```

**含义**：底层 IO 操作失败。这是一个"兜底"错误变体，涵盖所有 IO 相关的错误。

**触发场景**：

| 来源 | 典型 `io::ErrorKind` | 说明 |
|------|---------------------|------|
| Pipe 连接 | `NotFound` | Named Pipe 不存在（mihomo 未启动或 pipe 名不匹配） |
| Pipe 连接 | `ConnectionRefused` | 多次重试后仍无法连接到 pipe |
| 请求超时 | `TimedOut` | 请求在超时时间内未完成 |
| 管道断开 | `BrokenPipe` | 服务端关闭了连接 |
| 响应解析 | `InvalidData` | HTTP 响应格式异常 / JSON 反序列化失败 |
| 进程操作 | 各种 | spawn / kill 进程时的系统错误 |
| 流式读取 | `Other` | 流式请求收到非 2xx HTTP 状态码 |

**附带数据**：`std::io::Error` — 包含 `ErrorKind` 和描述信息

**处理建议**：
根据 `io::ErrorKind` 细分处理（见 [常见 IO 错误细分](#常见-io-错误细分)）。

**示例**：

```rust
match mgr.get_version().await {
    Ok(ver) => println!("version = {}", ver.version),
    Err(ProcessError::Io(e)) => {
        match e.kind() {
            io::ErrorKind::NotFound => {
                eprintln!("管道不存在，mihomo 是否已启动？");
            }
            io::ErrorKind::TimedOut => {
                eprintln!("请求超时，mihomo 可能无响应");
            }
            io::ErrorKind::BrokenPipe => {
                eprintln!("连接中断，mihomo 可能已退出");
            }
            _ => {
                eprintln!("IO 错误: {} (kind={:?})", e, e.kind());
            }
        }
    }
    Err(e) => return Err(e.into()),
}
```

---

### `NotReady`

```rust
NotReady(u32)
```

**含义**：在指定的重试次数内，mihomo API 始终未就绪。

**触发场景**：
- `MihomoManager::wait_ready()` 或 `start_and_wait()` 超过 `max_retries` 次后仍无法成功调用 `GET /`

**附带数据**：`u32` — 实际重试的总次数

**处理建议**：

| 方案 | 说明 |
|------|------|
| 增大重试参数 | 增大 `max_retries` 或 `interval` |
| 检查配置 | 确认 `external-controller-pipe` 已正确设置 |
| 检查 pipe 名称 | 确认 `PipeTransport` 和 mihomo 使用相同的 pipe 名称 |
| 检查进程 | 确认 mihomo 进程仍在运行且没有报错 |
| 重启 | 调用 `mgr.restart().await` 后重试 |

**示例**：

```rust
match mgr.start_and_wait(20, Duration::from_millis(500)).await {
    Ok(pid) => println!("就绪, PID = {}", pid),
    Err(ProcessError::NotReady(attempts)) => {
        eprintln!("mihomo API 在 {} 次重试后仍未就绪", attempts);
        eprintln!("可能的原因:");
        eprintln!("  1. 配置文件中未设置 external-controller-pipe");
        eprintln!("  2. PipeTransport 的 pipe 名称与配置不匹配");
        eprintln!("  3. mihomo 启动过程中遇到错误");
    }
    Err(e) => return Err(e.into()),
}
```

---

## 各层错误来源

### 进程管理层

`MihomoManager` 的进程管理方法产生的错误：

| 方法 | 可能的错误 |
|------|-----------|
| `start()` | `BinaryNotFound`、`ConfigNotFound`、`AlreadyRunning`、`Io`（spawn 失败） |
| `stop()` | `NotRunning`、`Io`（kill 失败） |
| `restart()` | `BinaryNotFound`、`ConfigNotFound`、`Io` |
| `wait_ready()` | `NotRunning`、`NotReady` |
| `start_and_wait()` | 以上所有 |

---

### 传输层（PipeTransport）

`PipeTransport` 的 HTTP 方法产生的错误：

| 方法 | 可能的错误 |
|------|-----------|
| `get` / `put` / `post` / `patch` / `delete` | `Io`（连接失败、超时、读写错误、解析失败） |
| `stream_get` | `Io`（连接失败、写入失败） |

**所有 PipeTransport 错误都是 `Io` 变体**，具体原因通过 `io::ErrorKind` 和错误消息区分。

---

### API 方法层

`MihomoManager` 上封装的 REST API 方法（如 `get_version`、`get_proxies` 等）：

| 错误来源 | `ProcessError` 变体 | 说明 |
|----------|---------------------|------|
| Pipe 传输失败 | `Io` | 连接/超时/断开等 |
| JSON 反序列化失败 | `Io`（`InvalidData`） | 响应 body 不是预期的 JSON 格式 |

> 📌 API 方法**不会**返回 `BinaryNotFound` / `ConfigNotFound` / `AlreadyRunning` /
> `NotRunning` / `NotReady`。这些错误仅来自进程管理层。

---

### 流式读取层（PipeStream）

`PipeStream<T>` 的 `poll_next` 产出的错误：

| 场景 | `ProcessError` 变体 | 说明 |
|------|---------------------|------|
| HTTP 非 2xx 状态码 | `Io`（`Other`） | 消息包含 HTTP 状态码和 body |
| 管道读取错误（非 BrokenPipe） | `Io` | 底层 IO 错误 |
| BrokenPipe / EOF | *(不是错误)* | 流正常结束，返回 `None` |
| JSON 解析失败 | *(不是错误)* | 该行被静默跳过 |

> 📌 **流级 JSON 解析失败不会产生错误**。无法反序列化的行会被静默跳过，
> 这是为了容忍非 JSON 内容（如 chunked 编码的长度行、空行等）。

---

## 常见 IO 错误细分

`ProcessError::Io` 是最常见的错误变体，以下是按 `io::ErrorKind` 细分的常见情况：

| `ErrorKind` | 典型消息 | 含义 | 常见原因 |
|-------------|----------|------|----------|
| `NotFound` | "系统找不到指定的文件" | Pipe 不存在 | mihomo 未启动，或 pipe 名称不匹配 |
| `ConnectionRefused` | "failed to connect to pipe: ..." | 连接被拒 | 3 次重试后仍无法连接 |
| `TimedOut` | "request timed out: GET /version" | 请求超时 | mihomo 无响应，或超时时间过短 |
| `BrokenPipe` | — | 连接中断 | mihomo 进程退出或 restart |
| `InvalidData` | "failed to parse response JSON: ..." | 数据格式异常 | 响应不是预期的 JSON 结构 |
| `InvalidData` | "failed to parse HTTP response: ..." | HTTP 格式异常 | 收到的不是合法的 HTTP 响应 |
| `Other` | "streaming request failed with HTTP 404: ..." | 流式请求失败 | 端点返回错误状态码 |
| `PermissionDenied` | — | 权限不足 | 无权访问 pipe 或启动进程 |

### 检查 ErrorKind 的辅助函数

```rust
fn classify_io_error(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::NotFound => "管道不存在（mihomo 未启动？）",
        std::io::ErrorKind::ConnectionRefused => "连接被拒（无法连接到管道）",
        std::io::ErrorKind::TimedOut => "请求超时",
        std::io::ErrorKind::BrokenPipe => "连接中断（mihomo 可能已退出）",
        std::io::ErrorKind::InvalidData => "数据格式错误",
        std::io::ErrorKind::PermissionDenied => "权限不足",
        _ => "未知 IO 错误",
    }
}

// 使用
match mgr.get_version().await {
    Err(ProcessError::Io(ref e)) => {
        eprintln!("{}: {}", classify_io_error(e), e);
    }
    // ...
}
```

---

## 错误处理模式

### 基本 match

最直接的错误处理方式——对每个变体分别处理：

```rust
use mihomo_sdk::{ProcessError, MihomoManager};

async fn handle(mgr: &MihomoManager) {
    match mgr.start().await {
        Ok(pid) => {
            println!("启动成功, PID = {}", pid);
        }
        Err(ProcessError::BinaryNotFound(path)) => {
            eprintln!("二进制文件不存在: {:?}", path);
        }
        Err(ProcessError::ConfigNotFound(path)) => {
            eprintln!("配置文件不存在: {:?}", path);
        }
        Err(ProcessError::AlreadyRunning(pid)) => {
            println!("已在运行, PID = {}", pid);
        }
        Err(ProcessError::NotRunning) => {
            // start() 不会返回此错误，但为了完整性
            unreachable!();
        }
        Err(ProcessError::Io(e)) => {
            eprintln!("IO 错误: {}", e);
        }
        Err(ProcessError::NotReady(n)) => {
            // start() 不会返回此错误
            unreachable!();
        }
    }
}
```

---

### 按层分类处理

根据错误来源将错误分为"可预期"和"不可预期"两类：

```rust
async fn safe_start(mgr: &MihomoManager) -> Result<u32, String> {
    match mgr.start_and_wait(20, Duration::from_millis(500)).await {
        Ok(pid) => Ok(pid),

        // 可预期的配置错误 — 用户可以修复
        Err(ProcessError::BinaryNotFound(p)) =>
            Err(format!("请将 mihomo.exe 放到 {:?}", p)),
        Err(ProcessError::ConfigNotFound(p)) =>
            Err(format!("请创建配置文件 {:?}", p)),

        // 进程状态错误 — 通常可以忽略或重试
        Err(ProcessError::AlreadyRunning(pid)) => Ok(pid), // 当作成功
        Err(ProcessError::NotRunning) =>
            Err("mihomo 进程在启动过程中意外退出".to_string()),

        // API 就绪超时 — 可能是配置问题
        Err(ProcessError::NotReady(n)) =>
            Err(format!("API 在 {} 次重试后仍未就绪，请检查配置", n)),

        // IO 错误 — 底层问题
        Err(ProcessError::Io(e)) =>
            Err(format!("系统错误: {}", e)),
    }
}
```

---

### 可恢复 vs 不可恢复

| 错误 | 可恢复？ | 建议操作 |
|------|---------|----------|
| `BinaryNotFound` | ❌ | 通知用户，需要手动修复路径 |
| `ConfigNotFound` | ⚠️ | 可自动创建默认配置后重试 |
| `AlreadyRunning` | ✅ | 忽略（目标已达成）或 restart |
| `NotRunning` | ⚠️ | 取决于场景：stop 时可忽略，wait_ready 时需重新 start |
| `Io`（NotFound） | ⚠️ | mihomo 未启动，等待后重试 |
| `Io`（TimedOut） | ✅ | 增大超时后重试 |
| `Io`（BrokenPipe） | ✅ | 重新连接后重试 |
| `Io`（InvalidData） | ❌ | 响应格式不兼容，通常是版本不匹配 |
| `NotReady` | ⚠️ | 增大重试次数后重试，或检查配置 |

```rust
async fn resilient_call(mgr: &MihomoManager) -> Result<String, ProcessError> {
    for attempt in 1..=3 {
        match mgr.get_version().await {
            Ok(ver) => return Ok(ver.version),
            Err(ProcessError::Io(ref e))
                if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::BrokenPipe =>
            {
                eprintln!("尝试 {}/3 失败: {}, 重试中...", attempt, e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            Err(e) => return Err(e), // 不可恢复，直接返回
        }
    }
    Err(ProcessError::Io(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "3 次重试后仍然失败",
    )))
}
```

---

### 重试策略

SDK 本身不内置 API 调用层的重试（传输层的 pipe 连接有 3 次重试）。
以下是一个通用的重试 helper：

```rust
use std::time::Duration;
use mihomo_sdk::ProcessError;

/// 带指数退避的重试。
async fn retry<T, F, Fut>(
    max_attempts: u32,
    base_delay: Duration,
    mut f: F,
) -> Result<T, ProcessError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ProcessError>>,
{
    let mut last_err = None;

    for attempt in 0..max_attempts {
        match f().await {
            Ok(val) => return Ok(val),
            Err(ProcessError::Io(e))
                if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::BrokenPipe
                || e.kind() == std::io::ErrorKind::NotFound =>
            {
                let delay = base_delay * 2u32.pow(attempt.min(4));
                eprintln!(
                    "重试 {}/{}: {} (等待 {:?})",
                    attempt + 1, max_attempts, e, delay
                );
                last_err = Some(ProcessError::Io(e));
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e), // 不可恢复
        }
    }

    Err(last_err.unwrap())
}

// 使用
let version = retry(3, Duration::from_millis(500), || {
    mgr.get_version()
}).await?;
```

---

### 日志记录

`mihomo_sdk` 内部使用 `log` crate 输出日志。将 `ProcessError` 与日志结合使用：

```rust
use log::{info, warn, error};

async fn monitored_start(mgr: &MihomoManager) -> Result<u32, ProcessError> {
    info!("正在启动 mihomo...");

    match mgr.start_and_wait(20, Duration::from_millis(500)).await {
        Ok(pid) => {
            info!("mihomo 启动成功, PID = {}", pid);
            Ok(pid)
        }
        Err(ref e @ ProcessError::BinaryNotFound(ref p)) => {
            error!("二进制文件不存在: {:?}", p);
            Err(e.clone()) // 注意：ProcessError 未实现 Clone，需要重新构造
        }
        Err(ProcessError::NotReady(n)) => {
            warn!("API 就绪超时 ({} 次重试)", n);
            Err(ProcessError::NotReady(n))
        }
        Err(ProcessError::Io(e)) => {
            error!("IO 错误: {} (kind={:?})", e, e.kind());
            Err(ProcessError::Io(e))
        }
        Err(e) => {
            warn!("启动异常: {}", e);
            Err(e)
        }
    }
}
```

> 📌 `ProcessError` **没有**实现 `Clone` trait（因为 `io::Error` 不可克隆）。
> 在需要保留错误的场景中，需要按值传递或重新构造。

---

## 与 mihomo API 错误的关系

mihomo 在请求失败时返回非 2xx HTTP 状态码和 JSON 错误体：

```json
{"message": "proxy not found"}
```

SDK 中定义了 `ApiError` 结构体来表示这种错误：

```rust
pub struct ApiError {
    pub message: String,
}
```

**但是**，SDK 的高层 API 方法（如 `get_proxy`、`select_proxy` 等）**不会**
自动将 HTTP 错误码转换为 `ApiError`。它们的行为是：

1. 发送请求 → 得到 `HttpResponse`（status + body）
2. 尝试将 body 反序列化为预期的响应类型
3. 如果反序列化成功 → 返回 `Ok(T)`
4. 如果反序列化失败（例如 body 是 `{"message": "not found"}` 而不是预期格式）→ 返回 `Err(ProcessError::Io)` 并附带 `InvalidData` 错误

**手动获取 API 错误信息**：

```rust
// 使用低层 transport 获取原始响应
let resp = mgr.api().get("/proxies/不存在的代理").await?;

if resp.status == 404 {
    if let Ok(err) = serde_json::from_str::<mihomo_sdk::api::ApiError>(&resp.body) {
        eprintln!("API 错误: {}", err.message);
    }
} else if resp.status == 200 {
    let proxy: mihomo_sdk::api::ProxyInfo = serde_json::from_str(&resp.body)?;
    println!("代理类型: {}", proxy.proxy_type);
}
```

**使用高层 API 时的错误表现**：

```rust
match mgr.get_proxy("不存在的代理").await {
    Ok(proxy) => println!("类型: {}", proxy.proxy_type),
    Err(ProcessError::Io(e)) if e.kind() == std::io::ErrorKind::InvalidData => {
        // mihomo 返回了 404 + {"message": "not found"}
        // SDK 尝试将其反序列化为 ProxyInfo 失败
        eprintln!("代理可能不存在: {}", e);
    }
    Err(e) => eprintln!("其他错误: {}", e),
}
```

> 📌 **未来改进方向**：可以考虑在 API 层增加对 HTTP 状态码的检查，
> 在非 2xx 时自动解析 `ApiError` 并返回更具描述性的错误类型。

---

## 在异步上下文中的使用

### 与 `?` 运算符

`ProcessError` 实现了 `std::error::Error`，可以与 `?` 运算符配合使用：

```rust
async fn do_work(mgr: &MihomoManager) -> Result<(), ProcessError> {
    mgr.start_and_wait(20, Duration::from_millis(500)).await?;

    let version = mgr.get_version().await?;
    println!("version = {}", version.version);

    let configs = mgr.get_configs().await?;
    println!("mode = {}", configs.mode);

    mgr.stop().await?;
    Ok(())
}
```

### 与 `Box<dyn Error>`

```rust
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");
    let pid = mgr.start_and_wait(20, Duration::from_millis(500)).await?;
    // ProcessError 自动转换为 Box<dyn Error>
    println!("PID = {}", pid);
    mgr.stop().await?;
    Ok(())
}
```

### 与 `anyhow`

```rust
use anyhow::{Context, Result};

async fn do_work(mgr: &MihomoManager) -> Result<()> {
    mgr.start_and_wait(20, Duration::from_millis(500))
        .await
        .context("启动 mihomo 失败")?;

    let ver = mgr.get_version()
        .await
        .context("获取版本信息失败")?;

    println!("version = {}", ver.version);
    Ok(())
}
```

### From<io::Error> 转换

`ProcessError` 实现了 `From<std::io::Error>`，因此在返回 `Result<_, ProcessError>`
的函数中，`io::Error` 会自动转换为 `ProcessError::Io`：

```rust
async fn read_config_and_start(
    mgr: &MihomoManager,
    path: &str,
) -> Result<u32, ProcessError> {
    // io::Error 自动转换为 ProcessError::Io
    let config_content = tokio::fs::read_to_string(path).await?;
    println!("配置文件大小: {} 字节", config_content.len());

    let pid = mgr.start().await?;
    Ok(pid)
}
```

---

## 自定义错误转换

如果你的应用有自己的错误类型，可以实现 `From<ProcessError>` 转换：

```rust
#[derive(Debug)]
enum AppError {
    MihomoProcess(mihomo_sdk::ProcessError),
    Config(String),
    Other(String),
}

impl From<mihomo_sdk::ProcessError> for AppError {
    fn from(e: mihomo_sdk::ProcessError) -> Self {
        AppError::MihomoProcess(e)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::MihomoProcess(e) => write!(f, "mihomo 错误: {}", e),
            AppError::Config(msg) => write!(f, "配置错误: {}", msg),
            AppError::Other(msg) => write!(f, "其他错误: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

// 使用
async fn app_logic(mgr: &MihomoManager) -> Result<(), AppError> {
    mgr.start_and_wait(20, Duration::from_millis(500)).await?;
    // ProcessError 自动转换为 AppError::MihomoProcess
    Ok(())
}
```

---

## 错误处理速查表

| 你遇到了... | 可能原因 | 建议操作 |
|-------------|----------|----------|
| `BinaryNotFound` | 路径拼写错误、文件不存在 | 检查 `binary_path`，确认文件存在 |
| `ConfigNotFound` | 路径拼写错误、文件不存在 | 检查 `config_path`，确认文件存在 |
| `AlreadyRunning` | 重复调用 `start()` | 使用 `restart()` 或忽略 |
| `NotRunning` | 进程未启动或已退出 | 调用 `start()` 或检查为什么进程退出 |
| `Io`（NotFound） | Pipe 不存在 | 确认 mihomo 已启动且 pipe 名称匹配 |
| `Io`（TimedOut） | 请求超时 | 增大超时(`with_timeout`)，检查 mihomo 状态 |
| `Io`（BrokenPipe） | mihomo 关闭了连接 | mihomo 可能 restart 了，重新连接 |
| `Io`（InvalidData） | JSON 解析失败 | 检查 mihomo 版本兼容性 |
| `Io`（PermissionDenied） | 权限不足 | 以管理员身份运行 |
| `NotReady` | API 就绪超时 | 增大重试参数，检查配置中的 pipe 设置 |

---

## 相关文档

- [进程管理](./process-management.md) — 进程管理方法及其错误
- [传输层](./transport.md) — `PipeTransport` 的错误处理
- [流式读取](./streaming.md) — `PipeStream` 的错误处理
- [REST API 参考](./api-reference.md) — 各 API 方法的错误场景