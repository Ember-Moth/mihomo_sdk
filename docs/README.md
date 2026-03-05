# mihomo_sdk 文档

> **版本**: 0.1.0  
> **平台**: Windows（Named Pipe 传输）  
> **运行时**: Tokio 异步

---

## 文档目录

| 文档 | 说明 |
|------|------|
| [快速开始](./getting-started.md) | 安装依赖、最小示例、5 分钟上手 |
| [进程管理](./process-management.md) | `MihomoManager` 生命周期：创建、启动、停止、重启、配置参数 |
| [传输层](./transport.md) | `PipeTransport` 与 `HttpResponse`：Named Pipe 上的 HTTP 通信 |
| [流式读取](./streaming.md) | `PipeStream<T>`：异步 Stream 适配器，实时订阅 traffic / memory / logs / connections |
| [REST API 参考](./api-reference.md) | 全部 35+ 个封装方法的完整签名、参数、返回值、示例与注意事项 |
| [数据模型](./models.md) | 所有请求/响应结构体的字段说明、JSON 映射、序列化细节 |
| [错误处理](./error-handling.md) | `ProcessError` 枚举、常见错误场景与处理建议 |
| [架构设计](./architecture.md) | 分层架构、模块关系、设计决策与 mihomo 源码对照 |

---

## 项目结构

```text
mihomo_sdk/
├── Cargo.toml
├── mihomo.exe                    # mihomo 二进制（不随 crate 分发）
├── src/
│   ├── lib.rs                    # 入口：MihomoManager, ProcessError, ProcessStatus
│   └── api/
│       ├── mod.rs                # 模块导出
│       ├── transport.rs          # PipeTransport — Named Pipe HTTP 传输层
│       ├── stream.rs             # PipeStream<T> — 异步流式读取器
│       ├── mihomo.rs             # 35+ 个 REST API 封装方法
│       └── models.rs             # 全部请求/响应数据结构
├── tests/
│   ├── fixtures/
│   │   └── test_config.yaml      # 集成测试用最小配置
│   └── stream_integration.rs     # 流式端点集成测试（真实 mihomo.exe）
└── docs/                         # ← 你正在阅读的文档
    ├── README.md
    ├── getting-started.md
    ├── process-management.md
    ├── transport.md
    ├── streaming.md
    ├── api-reference.md
    ├── models.md
    ├── error-handling.md
    └── architecture.md
```

---

## 依赖关系

```text
mihomo_sdk
├── tokio          1.x    (process, sync, rt, macros, time, net, io-util)
├── serde          1.x    (derive)
├── serde_json     1.x
├── httparse       1.x
├── thiserror      2.x
├── log            0.4
├── futures-core   0.3
└── pin-project-lite 0.2
```

---

## 快速导航

- **我想 5 分钟跑通** → [快速开始](./getting-started.md)
- **我想管理 mihomo 进程** → [进程管理](./process-management.md)
- **我想调用某个具体 API** → [REST API 参考](./api-reference.md)
- **我想实时监控流量/日志** → [流式读取](./streaming.md)
- **我想了解某个返回结构体** → [数据模型](./models.md)
- **我遇到了错误** → [错误处理](./error-handling.md)
- **我想了解内部设计** → [架构设计](./architecture.md)