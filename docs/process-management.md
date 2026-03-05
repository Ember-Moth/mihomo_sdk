# 进程管理

`MihomoManager` 是 SDK 的核心入口，负责 mihomo 二进制进程的完整生命周期管理，
同时作为所有 REST API 调用的宿主对象。

---

## 目录

- [创建管理器](#创建管理器)
  - [`MihomoManager::new`](#mihomomanagernew)
  - [`MihomoManager::with_transport`](#mihomomanagerwith_transport)
- [配置启动参数](#配置启动参数)
  - [配置文件标志](#配置文件标志)
  - [工作目录 (`-d`)](#工作目录--d)
  - [Named Pipe 地址 (`-ext-ctl-pipe`)](#named-pipe-地址--ext-ctl-pipe)
  - [API 密钥 (`-secret`)](#api-密钥--secret)
  - [额外参数](#额外参数)
  - [Kill-on-Drop 行为](#kill-on-drop-行为)
- [路径管理](#路径管理)
- [生命周期控制](#生命周期控制)
  - [`start`](#start)
  - [`stop`](#stop)
  - [`restart`](#restart)
  - [`status` / `is_running`](#status--is_running)
  - [`wait_ready`](#wait_ready)
  - [`start_and_wait`](#start_and_wait)
- [API 访问器](#api-访问器)
- [Clone 与并发](#clone-与并发)
- [Drop 行为](#drop-行为)
- [完整示例](#完整示例)

---

## 创建管理器

### `MihomoManager::new`

```rust
pub fn new(
    binary_path: impl Into<PathBuf>,
    config_path: impl Into<PathBuf>,
) -> Self
```

使用默认配置创建管理器。

| 参数 | 说明 |
|------|------|
| `binary_path` | mihomo 可执行文件路径，如 `"./mihomo.exe"` |
| `config_path` | 配置文件路径，如 `"./config.yaml"` |

**默认值**：

| 设置项 | 默认值 |
|--------|--------|
| 配置文件标志 | `-f` |
| Pipe 名称 | `\\.\pipe\mihomo` |
| 请求超时 | 10 秒 |
| Secret | 无 |
| 工作目录 | 无（不传 `-d`） |
| ext-ctl-pipe 覆盖 | 无 |
| 额外参数 | 无 |
| kill_on_drop | `true` |

**示例**：

```rust
let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");
```

---

### `MihomoManager::with_transport`

```rust
pub fn with_transport(
    binary_path: impl Into<PathBuf>,
    config_path: impl Into<PathBuf>,
    transport: PipeTransport,
) -> Self
```

使用自定义 `PipeTransport` 创建管理器。当你需要自定义 pipe 名称、超时或 secret 时使用。

**示例**：

```rust
use mihomo_sdk::{MihomoManager, PipeTransport};
use std::time::Duration;

let transport = PipeTransport::new()
    .with_pipe_name(r"\\.\pipe\my_mihomo")
    .with_timeout(Duration::from_secs(30))
    .with_secret("my_api_key");

let mgr = MihomoManager::with_transport(
    "./mihomo.exe",
    "./config.yaml",
    transport,
);
```

---

## 配置启动参数

以下方法配置的是 mihomo 进程的**命令行参数**。所有设置均为异步方法（需要获取内部锁），
且可以在进程运行时修改——修改后将在下次 `start()` / `restart()` 时生效。

### 配置文件标志

```rust
pub async fn set_config_flag(&self, flag: impl Into<String>)
```

设置配置文件的命令行参数标志。默认为 `-f`。

在极少数情况下你可能需要改成其他值（如测试场景中使用 `-n` 传给 ping），
但绝大多数情况下不需要修改。

**示例**：

```rust
mgr.set_config_flag("-f").await; // 默认值，通常无需调用
```

---

### 工作目录 (`-d`)

```rust
pub async fn set_home_dir(&self, dir: impl Into<PathBuf>)
pub async fn clear_home_dir(&self)
```

设置或清除 mihomo 的工作/配置目录。对应 mihomo 的 `-d` 命令行参数。

mihomo 会在此目录下查找：
- 默认配置文件（当不指定 `-f` 时）
- GeoIP / GeoSite 数据库
- 缓存文件

**来源**：`main.go` — `flag.StringVar(&homeDir, "d", ...)`

**示例**：

```rust
mgr.set_home_dir("C:\\mihomo\\data").await;

// 清除设置，不再传 -d 参数
mgr.clear_home_dir().await;
```

---

### Named Pipe 地址 (`-ext-ctl-pipe`)

```rust
pub async fn set_ext_ctl_pipe(&self, pipe_addr: impl Into<String>)
pub async fn clear_ext_ctl_pipe(&self)
```

设置或清除命令行覆盖的 Named Pipe 地址。对应 mihomo 的 `-ext-ctl-pipe` 参数。

设置此值后，mihomo 启动时会用它覆盖配置文件中的 `external-controller-pipe` 项。

**来源**：`main.go` — `flag.StringVar(&externalControllerPipe, "ext-ctl-pipe", ...)`

> ⚠️ **重要**：
> 1. Pipe 地址必须以 `\\.\pipe\` 开头
> 2. 此处设置的地址必须与 `PipeTransport` 中的 pipe 名称**完全一致**，否则 SDK 无法连接

**示例**：

```rust
let pipe = r"\\.\pipe\my_mihomo";

// 同时设置启动参数和传输层
mgr.set_ext_ctl_pipe(pipe).await;
// PipeTransport 也需要用相同的 pipe 名
// （推荐在创建时通过 with_transport 统一设置）
```

---

### API 密钥 (`-secret`)

```rust
pub async fn set_secret(&self, secret: impl Into<String>)
pub async fn clear_secret(&self)
```

设置或清除 API 密钥覆盖。对应 mihomo 的 `-secret` 参数。

**来源**：`main.go` — `flag.StringVar(&secret, "secret", ...)`

> 📌 **关于 Named Pipe 通道的 Secret**：
>
> 根据 mihomo 源码（`hub/route/server.go` 的 `startPipe` 函数），
> Named Pipe 通道在服务端**不校验 secret**——`startPipe` 向 router 传入空 secret。
> 因此，通过 pipe 访问 API 通常不需要 Bearer token。
>
> 此参数主要影响 TCP/TLS 控制器通道（`external-controller`），不影响 pipe 通道。
> 但设置它不会有副作用。

**示例**：

```rust
mgr.set_secret("my_secure_token").await;

// 清除
mgr.clear_secret().await;
```

---

### 额外参数

```rust
pub async fn add_extra_arg(&self, arg: impl Into<String>)
pub async fn set_extra_args(&self, args: Vec<String>)
pub async fn clear_extra_args(&self)
```

管理追加到命令行末尾的额外参数。

mihomo 支持的其他参数（来源：`mihomo -h`）：

| 参数 | 说明 |
|------|------|
| `-m` | 启用 geodata 模式 |
| `-ext-ctl <addr>` | 覆盖 TCP 外部控制器地址 |
| `-ext-ctl-unix <addr>` | 覆盖 Unix socket 控制器地址 |
| `-ext-ui <dir>` | 覆盖外部 UI 目录 |
| `-t` | 测试配置文件并退出 |
| `-v` | 显示版本信息 |

**示例**：

```rust
// 逐个添加
mgr.add_extra_arg("-m").await;
mgr.add_extra_arg("-ext-ctl").await;
mgr.add_extra_arg("127.0.0.1:9090").await;

// 或一次性设置全部
mgr.set_extra_args(vec![
    "-m".to_string(),
    "-ext-ctl".to_string(),
    "127.0.0.1:9090".to_string(),
]).await;

// 清除所有额外参数
mgr.clear_extra_args().await;
```

---

### Kill-on-Drop 行为

```rust
pub async fn set_kill_on_drop(&self, kill: bool)
```

设置当 `MihomoManager`（的最后一个 Clone）被 drop 时，是否自动 kill 子进程。

| 值 | 行为 |
|----|------|
| `true`（默认） | Drop 时自动终止 mihomo 进程 |
| `false` | Drop 时保留 mihomo 进程继续运行 |

**示例**：

```rust
// 让 mihomo 在程序退出后继续运行
mgr.set_kill_on_drop(false).await;
```

---

## 路径管理

```rust
pub async fn binary_path(&self) -> PathBuf
pub async fn config_path(&self) -> PathBuf
pub async fn set_binary_path(&self, path: impl Into<PathBuf>)
pub async fn set_config_path(&self, path: impl Into<PathBuf>)
```

获取或更新二进制/配置文件路径。进程运行时也可以更新，下次 `start()` / `restart()` 生效。

**示例**：

```rust
// 查看当前路径
let bin = mgr.binary_path().await;
let cfg = mgr.config_path().await;
println!("binary: {:?}, config: {:?}", bin, cfg);

// 切换到新版本
mgr.set_binary_path("./mihomo_v2.exe").await;
mgr.restart().await?;
```

---

## 生命周期控制

### `start`

```rust
pub async fn start(&self) -> Result<u32, ProcessError>
```

启动 mihomo 进程。返回进程 PID。

**构建的命令行**：

```text
mihomo.exe -f /path/to/config.yaml [-d /home/dir] [-ext-ctl-pipe \\.\pipe\mihomo] [-secret xxx] [extra_args...]
```

**前置检查**：
1. 如果进程已在运行 → 返回 `ProcessError::AlreadyRunning(pid)`
2. 如果二进制文件不存在 → 返回 `ProcessError::BinaryNotFound(path)`
3. 如果配置文件不存在 → 返回 `ProcessError::ConfigNotFound(path)`

**示例**：

```rust
match mgr.start().await {
    Ok(pid) => println!("已启动, PID = {}", pid),
    Err(ProcessError::AlreadyRunning(pid)) => println!("已在运行, PID = {}", pid),
    Err(ProcessError::BinaryNotFound(p)) => eprintln!("找不到: {:?}", p),
    Err(ProcessError::ConfigNotFound(p)) => eprintln!("配置不存在: {:?}", p),
    Err(e) => eprintln!("启动失败: {}", e),
}
```

> ⚠️ `start()` 返回成功仅表示进程已启动，**不代表 API 已就绪**。
> 使用 `wait_ready()` 或 `start_and_wait()` 来等待 API 可用。

---

### `stop`

```rust
pub async fn stop(&self) -> Result<(), ProcessError>
```

停止（kill）mihomo 进程。

- 如果进程未运行 → 返回 `ProcessError::NotRunning`
- 如果进程已自行退出 → 清理状态，返回 `Ok(())`
- 正常情况 → 发送 kill 信号并等待进程退出

**示例**：

```rust
mgr.stop().await?;
println!("mihomo 已停止");
```

---

### `restart`

```rust
pub async fn restart(&self) -> Result<u32, ProcessError>
```

重启进程：先 `stop()` 再 `start()`。

- 如果当前没有运行 → 直接启动（等同于 `start()`）
- 如果正在运行 → 先停止再启动

返回新进程的 PID。

**示例**：

```rust
let new_pid = mgr.restart().await?;
println!("新 PID = {}", new_pid);

// 重启后需要重新等待 API 就绪
mgr.wait_ready(20, Duration::from_millis(500)).await?;
```

---

### `status` / `is_running`

```rust
pub async fn status(&self) -> ProcessStatus
pub async fn is_running(&self) -> bool
```

查询当前进程状态。

`ProcessStatus` 枚举：

```rust
pub enum ProcessStatus {
    Stopped,        // 未启动 / 已退出
    Running(u32),   // 正在运行, 附带 PID
}
```

**实现细节**：
内部调用 `child.try_wait()` 检测进程是否仍然存活。如果进程已退出但 `stop()` 未被调用，
`status()` 会自动清理内部状态。

**示例**：

```rust
match mgr.status().await {
    ProcessStatus::Running(pid) => println!("运行中, PID = {}", pid),
    ProcessStatus::Stopped => println!("已停止"),
}

if mgr.is_running().await {
    println!("进程活着");
}
```

---

### `wait_ready`

```rust
pub async fn wait_ready(
    &self,
    max_retries: u32,
    interval: Duration,
) -> Result<(), ProcessError>
```

等待 mihomo API 就绪。在 `start()` 之后调用。

**工作原理**：
1. 每次循环先检查进程是否仍然存活（如果已退出 → `ProcessError::NotRunning`）
2. 发送 `GET /` 请求（hello 端点）
3. 收到 HTTP 200 → 返回 `Ok(())`
4. 失败 → 等待 `interval` 后重试
5. 超过 `max_retries` 次 → 返回 `ProcessError::NotReady(max_retries)`

| 参数 | 说明 |
|------|------|
| `max_retries` | 最大重试次数 |
| `interval` | 每次重试之间的等待时间 |

**推荐值**：

| 场景 | max_retries | interval |
|------|-------------|----------|
| 本地开发 | 20 | 500ms |
| 生产环境 | 40 | 1s |
| CI 测试 | 30 | 500ms |

**示例**：

```rust
mgr.start().await?;
mgr.wait_ready(20, Duration::from_millis(500)).await?;
// 现在可以安全调用 API
```

---

### `start_and_wait`

```rust
pub async fn start_and_wait(
    &self,
    max_retries: u32,
    interval: Duration,
) -> Result<u32, ProcessError>
```

便捷方法，等价于 `start()` + `wait_ready()`。返回进程 PID。

**示例**：

```rust
let pid = mgr.start_and_wait(20, Duration::from_millis(500)).await?;
println!("mihomo 已就绪, PID = {}", pid);
```

---

## API 访问器

```rust
pub fn api(&self) -> &PipeTransport
```

获取底层 `PipeTransport` 的引用，用于直接发送原始 HTTP 请求。

当 SDK 尚未封装某个 API 端点，或你需要发送自定义请求时使用。

**示例**：

```rust
// 调用 SDK 尚未封装的端点
let resp = mgr.api().get("/some/new/endpoint").await?;
println!("status={}, body={}", resp.status, resp.body);

// 发送自定义 PATCH
let resp = mgr.api().patch("/configs", r#"{"tun":{"enable":true}}"#).await?;

// 流式请求
let stream = mgr.api().stream_get::<TrafficEntry>("/traffic").await?;
```

---

## Clone 与并发

`MihomoManager` 实现了 `Clone`。克隆是低成本的（内部共享 `Arc<Mutex<Inner>>`）。

所有克隆体共享同一个子进程和配置状态。可以安全地在多个 tokio 任务中并发使用：

```rust
let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");
mgr.start_and_wait(20, Duration::from_millis(500)).await?;

let mgr2 = mgr.clone();
let mgr3 = mgr.clone();

// 并发调用 API
let (version, configs, proxies) = tokio::join!(
    mgr.get_version(),
    mgr2.get_configs(),
    mgr3.get_proxies(),
);
```

> ⚠️ 注意：`PipeTransport` 也实现了 `Clone`，且是**无状态的**（每次请求建立新的 pipe 连接），
> 因此多个克隆体可以安全并发发送请求。

---

## Drop 行为

当 `MihomoManager` 的**最后一个克隆体**被 drop 时（即 `Arc` 引用计数归零），
`Inner` 的 `Drop` 实现会执行以下逻辑：

```text
if 子进程仍在运行:
    if kill_on_drop == true:
        发送 kill 信号（非阻塞，无法 await）
    else:
        仅打印警告日志，进程继续运行
```

> 📌 **注意**：Drop 中使用的是 `child.start_kill()`（非阻塞），不等待进程实际退出。
> 如果需要确保进程完全退出，请在 drop 前显式调用 `mgr.stop().await`。

---

## 完整示例

### 基本生命周期

```rust
use mihomo_sdk::{MihomoManager, ProcessStatus, ProcessError};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");

    // 配置启动参数
    mgr.set_home_dir("C:\\mihomo").await;
    mgr.set_ext_ctl_pipe(r"\\.\pipe\mihomo").await;

    // 启动并等待就绪
    let pid = mgr.start_and_wait(20, Duration::from_millis(500)).await?;
    println!("启动成功, PID = {}", pid);

    // 验证状态
    assert!(matches!(mgr.status().await, ProcessStatus::Running(_)));

    // 调用 API
    let ver = mgr.get_version().await?;
    println!("版本: {}", ver.version);

    // 重启
    let new_pid = mgr.restart().await?;
    mgr.wait_ready(20, Duration::from_millis(500)).await?;
    println!("重启成功, 新 PID = {}", new_pid);

    // 停止
    mgr.stop().await?;
    assert_eq!(mgr.status().await, ProcessStatus::Stopped);

    Ok(())
}
```

### 多实例管理

```rust
use mihomo_sdk::{MihomoManager, PipeTransport};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 实例 1
    let mgr1 = MihomoManager::with_transport(
        "./mihomo.exe",
        "./config1.yaml",
        PipeTransport::new().with_pipe_name(r"\\.\pipe\mihomo_1"),
    );
    mgr1.set_ext_ctl_pipe(r"\\.\pipe\mihomo_1").await;

    // 实例 2
    let mgr2 = MihomoManager::with_transport(
        "./mihomo.exe",
        "./config2.yaml",
        PipeTransport::new().with_pipe_name(r"\\.\pipe\mihomo_2"),
    );
    mgr2.set_ext_ctl_pipe(r"\\.\pipe\mihomo_2").await;

    // 并行启动
    let (r1, r2) = tokio::join!(
        mgr1.start_and_wait(20, Duration::from_millis(500)),
        mgr2.start_and_wait(20, Duration::from_millis(500)),
    );
    r1?;
    r2?;

    println!("实例1 版本: {}", mgr1.get_version().await?.version);
    println!("实例2 版本: {}", mgr2.get_version().await?.version);

    // 并行停止
    let (s1, s2) = tokio::join!(mgr1.stop(), mgr2.stop());
    s1?;
    s2?;

    Ok(())
}
```

### 守护进程模式（不随主程序退出）

```rust
use mihomo_sdk::MihomoManager;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");

    // 进程不随主程序退出
    mgr.set_kill_on_drop(false).await;

    mgr.start_and_wait(20, Duration::from_millis(500)).await?;
    println!("mihomo 已启动，退出程序后进程继续运行");

    // 不调用 stop()，直接退出
    Ok(())
}
```

---

## 相关文档

- [传输层](./transport.md) — `PipeTransport` 的配置与直接使用
- [REST API 参考](./api-reference.md) — 所有封装的 API 方法
- [错误处理](./error-handling.md) — `ProcessError` 各变体的含义与处理