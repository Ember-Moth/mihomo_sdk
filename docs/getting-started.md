# 快速开始

本指南帮助你在 5 分钟内完成 `mihomo_sdk` 的安装和第一次调用。

---

## 前置条件

| 条件 | 说明 |
|------|------|
| **操作系统** | Windows 10 / 11（Named Pipe 为 Windows 专有特性） |
| **Rust 工具链** | 1.70+（需要 `edition = "2021"` 支持） |
| **mihomo 二进制** | 下载 [mihomo releases](https://github.com/MetaCubeX/mihomo/releases)，取 `mihomo-windows-amd64.exe` 并重命名为 `mihomo.exe` |
| **配置文件** | 一个有效的 mihomo YAML 配置，至少包含 `external-controller-pipe` 项 |

---

## 1. 添加依赖

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
mihomo_sdk = { path = "../mihomo_sdk" }   # 或发布后用 version = "0.1.0"
tokio = { version = "1", features = ["full"] }
serde_json = "1"

# 如果需要使用流式 API（stream_traffic / stream_logs 等）：
futures-core = "0.3"
```

---

## 2. 准备最小配置文件

创建 `config.yaml`：

```yaml
# 最小可用配置
mixed-port: 7890
log-level: info

# 必须：启用 Named Pipe 控制器
external-controller-pipe: \\.\pipe\mihomo
```

> **关键**：`external-controller-pipe` 是 SDK 与 mihomo 通信的唯一通道。
> 不设置此项，SDK 将无法连接。

---

## 3. 最小示例

```rust
use mihomo_sdk::MihomoManager;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建管理器，指定 mihomo 可执行文件和配置文件路径
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");

    // 2. 启动进程并等待 API 就绪
    //    最多重试 20 次，每次间隔 500ms（共约 10 秒）
    let pid = mgr.start_and_wait(20, Duration::from_millis(500)).await?;
    println!("mihomo 已启动, PID = {}", pid);

    // 3. 调用 API
    let version = mgr.get_version().await?;
    println!("版本: {}", version.version);

    let configs = mgr.get_configs().await?;
    println!("混合端口: {}", configs.mixed_port);
    println!("运行模式: {}", configs.mode);

    // 4. 停止进程
    mgr.stop().await?;
    println!("mihomo 已停止");

    Ok(())
}
```

运行：

```powershell
cargo run
```

预期输出：

```text
mihomo 已启动, PID = 12345
版本: v1.19.20
混合端口: 7890
运行模式: rule
mihomo 已停止
```

---

## 4. 自定义 Pipe 名称

如果你的配置文件中使用了非默认的 pipe 名称，或者你想运行多个实例避免冲突，
需要让 `PipeTransport` 和 mihomo 启动参数保持一致：

```rust
use mihomo_sdk::{MihomoManager, PipeTransport};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipe_name = r"\\.\pipe\my_mihomo_instance";

    // 自定义传输层
    let transport = PipeTransport::new()
        .with_pipe_name(pipe_name)
        .with_timeout(Duration::from_secs(15));

    // 用自定义传输层创建管理器
    let mgr = MihomoManager::with_transport("./mihomo.exe", "./config.yaml", transport);

    // 通过命令行参数覆盖 pipe 地址（优先级高于配置文件）
    mgr.set_ext_ctl_pipe(pipe_name).await;

    let pid = mgr.start_and_wait(20, Duration::from_millis(500)).await?;
    println!("PID = {}", pid);

    let ver = mgr.get_version().await?;
    println!("version = {}", ver.version);

    mgr.stop().await?;
    Ok(())
}
```

---

## 5. 实时监控流量（流式 API）

```rust
use mihomo_sdk::MihomoManager;
use std::pin::pin;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");
    mgr.start_and_wait(20, Duration::from_millis(500)).await?;

    // 打开流量流
    let stream = mgr.stream_traffic().await?;
    let mut pinned = pin!(stream);

    println!("实时流量监控 (Ctrl+C 退出):");

    // 手动 poll 不依赖 tokio-stream
    use std::future::poll_fn;
    use std::task::Poll;
    use futures_core::Stream;

    loop {
        let item = poll_fn(|cx| match pinned.as_mut().poll_next(cx) {
            Poll::Ready(item) => Poll::Ready(item),
            Poll::Pending => Poll::Pending,
        })
        .await;

        match item {
            Some(Ok(entry)) => {
                println!(
                    "↑ {:>10} B/s  ↓ {:>10} B/s  (total ↑{} ↓{})",
                    entry.up, entry.down, entry.up_total, entry.down_total
                );
            }
            Some(Err(e)) => {
                eprintln!("流错误: {}", e);
                break;
            }
            None => {
                println!("流结束");
                break;
            }
        }
    }

    mgr.stop().await?;
    Ok(())
}
```

> **提示**：如果你的项目已经依赖 `tokio-stream` 或 `futures`，可以用它们提供的
> `StreamExt::next()` 替代上面手动 `poll_fn` 的写法，代码会更简洁：
>
> ```rust
> use tokio_stream::StreamExt;
>
> let stream = mgr.stream_traffic().await?;
> tokio::pin!(stream);
> while let Some(Ok(entry)) = stream.next().await {
>     println!("up={} down={}", entry.up, entry.down);
> }
> ```

---

## 6. 代理管理快速示例

```rust
use mihomo_sdk::MihomoManager;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");
    mgr.start_and_wait(20, Duration::from_millis(500)).await?;

    // 列出所有代理
    let proxies = mgr.get_proxies().await?;
    for (name, info) in &proxies.proxies {
        println!("  {} ({})", name, info.proxy_type);
    }

    // 列出所有策略组
    let groups = mgr.get_groups().await?;
    for group in &groups.proxies {
        println!(
            "策略组 [{}] 类型={} 当前={}",
            group.name, group.group_type, group.now
        );
    }

    // 在 Selector 策略组中切换代理
    // mgr.select_proxy("节点选择", "某节点").await?;

    // 测试延迟
    // let delay = mgr.test_proxy_delay("某节点", "https://www.google.com", 5000).await?;
    // println!("延迟: {}ms", delay.delay);

    mgr.stop().await?;
    Ok(())
}
```

---

## 7. 动态修改配置

```rust
use mihomo_sdk::MihomoManager;
use serde_json::json;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");
    mgr.start_and_wait(20, Duration::from_millis(500)).await?;

    // 修改混合端口
    mgr.patch_configs(json!({"mixed-port": 8890})).await?;

    // 切换运行模式
    mgr.patch_configs(json!({"mode": "global"})).await?;

    // 验证修改
    let configs = mgr.get_configs().await?;
    println!("新端口: {}", configs.mixed_port); // 8890
    println!("新模式: {}", configs.mode);        // global

    // 重新加载完整配置文件
    mgr.reload_configs("", "").await?;

    mgr.stop().await?;
    Ok(())
}
```

---

## 常见问题

### Q: 启动后 `wait_ready` 超时？

**原因**：mihomo 尚未完成初始化，或者配置文件中没有设置 `external-controller-pipe`。

**解决**：
1. 确认 `config.yaml` 中有 `external-controller-pipe: \\.\pipe\mihomo`
2. 增大重试次数和间隔：`mgr.wait_ready(40, Duration::from_secs(1)).await?`
3. 检查 mihomo 标准输出/错误输出中是否有启动失败的日志

### Q: 连接 pipe 报 "系统找不到指定的文件"？

**原因**：mihomo 进程尚未启动，或 pipe 名称不匹配。

**解决**：
1. 确认 `MihomoManager::start()` 返回成功
2. 确认 `PipeTransport` 的 pipe 名称与配置文件 / `-ext-ctl-pipe` 参数一致
3. 在 `start()` 后加一个小延迟或使用 `start_and_wait()`

### Q: `kill_on_drop` 是什么？

默认情况下，当 `MihomoManager` 被 drop 时会自动 kill 子进程。
如果你想让 mihomo 在你的程序退出后继续运行，设置：

```rust
mgr.set_kill_on_drop(false).await;
```

### Q: 能否连接一个已经在运行的 mihomo？

可以。只要你知道它的 pipe 地址，直接创建 `PipeTransport` 即可：

```rust
let transport = PipeTransport::new()
    .with_pipe_name(r"\\.\pipe\mihomo");

// 不需要 MihomoManager，直接用 transport 调用 API
let resp = transport.get("/version").await?;
println!("{}", resp.body);
```

如果你同时需要进程管理能力，用 `MihomoManager::with_transport()` 创建管理器，
但不调用 `start()` 即可。

---

## 下一步

- [进程管理](./process-management.md) — 深入了解启动参数、生命周期控制
- [REST API 参考](./api-reference.md) — 查看全部 35+ 个 API 方法
- [流式读取](./streaming.md) — 实时订阅 traffic / memory / logs
- [数据模型](./models.md) — 所有结构体字段详解