use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use log::{error, info, warn};
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub mod api;

pub use api::{HttpResponse, PipeTransport};

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

/// 进程运行状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    /// 未启动 / 已停止
    Stopped,
    /// 正在运行，附带 pid
    Running(u32),
}

/// 管理 mihomo 二进制进程的完整生命周期，并通过 Named Pipe 调用其 HTTP API。
///
/// 支持的 mihomo 命令行参数（源自 `main.go`）：
/// - `-f <config_file>` — 指定配置文件
/// - `-d <home_dir>` — 指定工作/配置目录
/// - `-ext-ctl-pipe <pipe_addr>` — 覆盖 named pipe 地址
/// - `-secret <secret>` — 覆盖 API 密钥
///
/// 典型用法：
/// ```ignore
/// let mgr = MihomoManager::new("./mihomo.exe", "./config.yaml");
/// mgr.start().await?;
/// mgr.wait_ready(10, Duration::from_millis(500)).await?;
///
/// let ver = mgr.get_version().await?;
/// println!("mihomo {}", ver.version);
///
/// mgr.stop().await?;
/// ```
#[derive(Clone)]
pub struct MihomoManager {
    inner: Arc<Mutex<Inner>>,
    transport: PipeTransport,
}

struct Inner {
    /// 可执行文件路径，例如 `./mihomo.exe`
    binary_path: PathBuf,
    /// 配置文件路径，传给 `-f`
    config_path: PathBuf,
    /// 配置文件参数标志，默认 `-f`
    config_flag: String,
    /// mihomo 工作目录（`-d` 参数），为 None 时不传
    home_dir: Option<PathBuf>,
    /// 覆盖 named pipe 地址（`-ext-ctl-pipe` 参数），为 None 时不传
    ext_ctl_pipe: Option<String>,
    /// 覆盖 API secret（`-secret` 参数），为 None 时不传
    secret: Option<String>,
    /// 额外的命令行参数
    extra_args: Vec<String>,
    /// 是否在 Drop 时自动 kill 子进程
    kill_on_drop: bool,
    /// 子进程句柄
    child: Option<Child>,
}

impl MihomoManager {
    /// 创建新的 MihomoManager。
    ///
    /// - `binary_path`: 可执行文件路径，例如 `./mihomo.exe`
    /// - `config_path`: 配置文件路径，例如 `/path/to/config.yaml`
    ///
    /// 默认使用 `-f` 作为配置文件参数标志，
    /// 默认连接 `\\.\pipe\mihomo` 作为 API 通道，
    /// 默认在 Drop 时自动 kill 子进程。
    pub fn new(binary_path: impl Into<PathBuf>, config_path: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                binary_path: binary_path.into(),
                config_path: config_path.into(),
                config_flag: "-f".to_string(),
                home_dir: None,
                ext_ctl_pipe: None,
                secret: None,
                extra_args: Vec::new(),
                kill_on_drop: true,
                child: None,
            })),
            transport: PipeTransport::new(),
        }
    }

    /// 使用自定义的 `PipeTransport` 创建 MihomoManager。
    ///
    /// 当需要自定义 pipe 名称、超时时间或 API secret 时使用此方法。
    pub fn with_transport(
        binary_path: impl Into<PathBuf>,
        config_path: impl Into<PathBuf>,
        transport: PipeTransport,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                binary_path: binary_path.into(),
                config_path: config_path.into(),
                config_flag: "-f".to_string(),
                home_dir: None,
                ext_ctl_pipe: None,
                secret: None,
                extra_args: Vec::new(),
                kill_on_drop: true,
                child: None,
            })),
            transport,
        }
    }

    /// 获取 API 传输层引用，用于直接调用 mihomo HTTP API。
    ///
    /// ```ignore
    /// let resp = mgr.api().get("/version").await?;
    /// let resp = mgr.api().patch("/configs", r#"{"mixed-port":7890}"#).await?;
    /// ```
    pub fn api(&self) -> &PipeTransport {
        &self.transport
    }

    // ── 配置 setters ──────────────────────────────────────────────────

    /// 设置配置文件的命令行标志（默认 `-f`）。
    pub async fn set_config_flag(&self, flag: impl Into<String>) {
        self.inner.lock().await.config_flag = flag.into();
    }

    /// 设置 mihomo 工作目录（对应 `-d` 参数）。
    ///
    /// Source: `main.go` — `flag.StringVar(&homeDir, "d", ...)`.
    /// mihomo 会在此目录下查找默认配置文件、缓存等。
    pub async fn set_home_dir(&self, dir: impl Into<PathBuf>) {
        self.inner.lock().await.home_dir = Some(dir.into());
    }

    /// 清除 home dir 设置（不再传 `-d` 参数）。
    pub async fn clear_home_dir(&self) {
        self.inner.lock().await.home_dir = None;
    }

    /// 设置覆盖的 named pipe 地址（对应 `-ext-ctl-pipe` 参数）。
    ///
    /// Source: `main.go` — `flag.StringVar(&externalControllerPipe, "ext-ctl-pipe", ...)`.
    /// 如果设置了此值，mihomo 启动时会用它覆盖配置文件中的 `external-controller-pipe`。
    ///
    /// **注意**：pipe 地址必须以 `\\.\pipe\` 开头，否则 mihomo 会报错。
    /// 同时需要确保 `PipeTransport` 的 pipe 名称与此一致。
    pub async fn set_ext_ctl_pipe(&self, pipe_addr: impl Into<String>) {
        self.inner.lock().await.ext_ctl_pipe = Some(pipe_addr.into());
    }

    /// 清除 ext-ctl-pipe 覆盖设置。
    pub async fn clear_ext_ctl_pipe(&self) {
        self.inner.lock().await.ext_ctl_pipe = None;
    }

    /// 设置覆盖的 API secret（对应 `-secret` 参数）。
    ///
    /// Source: `main.go` — `flag.StringVar(&secret, "secret", ...)`.
    ///
    /// **注意**：通过 named pipe 访问 API 时，mihomo 服务端**不校验** secret
    /// （`startPipe` 传空 secret 给 router）。因此此参数主要影响 TCP/TLS 通道。
    /// 但设置它不会有副作用。
    pub async fn set_secret(&self, secret: impl Into<String>) {
        self.inner.lock().await.secret = Some(secret.into());
    }

    /// 清除 secret 覆盖设置。
    pub async fn clear_secret(&self) {
        self.inner.lock().await.secret = None;
    }

    /// 添加一个额外的命令行参数。
    ///
    /// 例如 `-m`（geodata mode）、`-ext-ctl 127.0.0.1:9090` 等。
    /// 参数会追加在 `-f <config>` 之后。
    pub async fn add_extra_arg(&self, arg: impl Into<String>) {
        self.inner.lock().await.extra_args.push(arg.into());
    }

    /// 设置所有额外命令行参数（替换之前的）。
    pub async fn set_extra_args(&self, args: Vec<String>) {
        self.inner.lock().await.extra_args = args;
    }

    /// 清除所有额外命令行参数。
    pub async fn clear_extra_args(&self) {
        self.inner.lock().await.extra_args.clear();
    }

    /// 设置是否在 MihomoManager Drop 时自动 kill 子进程。默认为 `true`。
    ///
    /// 如果设为 `false`，当 MihomoManager 被 drop 时，mihomo 进程会继续运行。
    pub async fn set_kill_on_drop(&self, kill: bool) {
        self.inner.lock().await.kill_on_drop = kill;
    }

    // ── 路径 getters/setters ─────────────────────────────────────────

    /// 返回当前配置的二进制路径。
    pub async fn binary_path(&self) -> PathBuf {
        self.inner.lock().await.binary_path.clone()
    }

    /// 返回当前配置的配置文件路径。
    pub async fn config_path(&self) -> PathBuf {
        self.inner.lock().await.config_path.clone()
    }

    /// 更新二进制路径（进程运行时也可以更新，下次 start/restart 生效）。
    pub async fn set_binary_path(&self, path: impl Into<PathBuf>) {
        self.inner.lock().await.binary_path = path.into();
    }

    /// 更新配置文件路径。
    pub async fn set_config_path(&self, path: impl Into<PathBuf>) {
        self.inner.lock().await.config_path = path.into();
    }

    // ── 进程管理 ──────────────────────────────────────────────────────

    /// 启动进程。如果已在运行则返回错误。
    ///
    /// 构建的命令行形如：
    /// ```text
    /// mihomo.exe -f /path/to/config.yaml [-d /home/dir] [-ext-ctl-pipe \\.\pipe\mihomo] [-secret xxx] [extra_args...]
    /// ```
    pub async fn start(&self) -> Result<u32, ProcessError> {
        let mut inner = self.inner.lock().await;

        // 如果 child 还在，先检查是否真的还活着
        if let Some(ref mut child) = inner.child {
            match child.try_wait() {
                Ok(Some(_exited)) => {
                    // 已经退出了，清理掉
                    inner.child = None;
                }
                Ok(None) => {
                    // 还在跑
                    return Err(ProcessError::AlreadyRunning(child.id().unwrap_or(0)));
                }
                Err(e) => {
                    warn!("failed to query child status: {e}");
                    inner.child = None;
                }
            }
        }

        let binary = &inner.binary_path;
        let config = &inner.config_path;

        if !binary.exists() {
            return Err(ProcessError::BinaryNotFound(binary.clone()));
        }
        if !config.exists() {
            return Err(ProcessError::ConfigNotFound(config.clone()));
        }

        // 构建命令行参数
        let mut cmd = Command::new(binary);

        // -f <config_path>
        cmd.arg(&inner.config_flag).arg(config);

        // -d <home_dir>
        if let Some(ref home_dir) = inner.home_dir {
            cmd.arg("-d").arg(home_dir);
        }

        // -ext-ctl-pipe <pipe_addr>
        if let Some(ref pipe_addr) = inner.ext_ctl_pipe {
            cmd.arg("-ext-ctl-pipe").arg(pipe_addr);
        }

        // -secret <secret>
        if let Some(ref secret) = inner.secret {
            cmd.arg("-secret").arg(secret);
        }

        // extra args
        for arg in &inner.extra_args {
            cmd.arg(arg);
        }

        info!(
            "starting {} {} {}{}{}{}",
            binary.display(),
            inner.config_flag,
            config.display(),
            inner
                .home_dir
                .as_ref()
                .map(|d| format!(" -d {}", d.display()))
                .unwrap_or_default(),
            inner
                .ext_ctl_pipe
                .as_ref()
                .map(|p| format!(" -ext-ctl-pipe {}", p))
                .unwrap_or_default(),
            if inner.extra_args.is_empty() {
                String::new()
            } else {
                format!(" {}", inner.extra_args.join(" "))
            },
        );

        let child = cmd.spawn()?;

        let pid = child.id().unwrap_or(0);
        inner.child = Some(child);

        info!("process started, pid={pid}");
        Ok(pid)
    }

    /// 停止进程。
    pub async fn stop(&self) -> Result<(), ProcessError> {
        let mut inner = self.inner.lock().await;

        let child = inner.child.as_mut().ok_or(ProcessError::NotRunning)?;

        // 先看看是不是已经退了
        match child.try_wait() {
            Ok(Some(status)) => {
                info!("process already exited: {status}");
                inner.child = None;
                return Ok(());
            }
            Ok(None) => {} // 还在跑，继续 kill
            Err(e) => {
                warn!("failed to query child status: {e}");
            }
        }

        let pid = child.id().unwrap_or(0);
        info!("stopping process pid={pid}");

        kill_process(child).await?;

        inner.child = None;
        info!("process stopped");
        Ok(())
    }

    /// 重启进程：先停止再启动。如果当前没有运行则直接启动。
    pub async fn restart(&self) -> Result<u32, ProcessError> {
        // 尝试停止，忽略 NotRunning 错误
        match self.stop().await {
            Ok(()) => {}
            Err(ProcessError::NotRunning) => {}
            Err(e) => return Err(e),
        }
        self.start().await
    }

    /// 查询当前进程状态。
    pub async fn status(&self) -> ProcessStatus {
        let mut inner = self.inner.lock().await;

        let alive = match inner.child.as_mut() {
            None => false,
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => {
                    inner.child = None;
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    warn!("failed to query child status: {e}");
                    inner.child = None;
                    false
                }
            },
        };

        if alive {
            let pid = inner.child.as_ref().unwrap().id().unwrap_or(0);
            ProcessStatus::Running(pid)
        } else {
            ProcessStatus::Stopped
        }
    }

    /// 进程是否正在运行。
    pub async fn is_running(&self) -> bool {
        matches!(self.status().await, ProcessStatus::Running(_))
    }

    /// 等待 mihomo API 就绪。
    ///
    /// 在 `start()` 之后调用，反复尝试 `GET /` (hello endpoint) 直到成功，
    /// 或者超过最大重试次数后返回 `ProcessError::NotReady`。
    ///
    /// - `max_retries`: 最大重试次数
    /// - `interval`: 每次重试之间的间隔
    ///
    /// Source: `hub/route/server.go` — `hello` 返回 `{"hello":"mihomo"}`。
    ///
    /// ```ignore
    /// mgr.start().await?;
    /// mgr.wait_ready(20, Duration::from_millis(500)).await?;
    /// // API 已就绪，可以安全调用其他方法
    /// let ver = mgr.get_version().await?;
    /// ```
    pub async fn wait_ready(
        &self,
        max_retries: u32,
        interval: Duration,
    ) -> Result<(), ProcessError> {
        for attempt in 1..=max_retries {
            // 先检查进程是否还活着
            if !self.is_running().await {
                return Err(ProcessError::NotRunning);
            }

            match self.api().get("/").await {
                Ok(resp) if resp.status == 200 => {
                    info!("mihomo API ready after {} attempt(s)", attempt);
                    return Ok(());
                }
                Ok(resp) => {
                    warn!(
                        "wait_ready attempt {}/{}: unexpected status {}",
                        attempt, max_retries, resp.status
                    );
                }
                Err(e) => {
                    if attempt < max_retries {
                        info!(
                            "wait_ready attempt {}/{}: {} (retrying in {:?})",
                            attempt, max_retries, e, interval
                        );
                    } else {
                        warn!(
                            "wait_ready attempt {}/{}: {} (giving up)",
                            attempt, max_retries, e
                        );
                    }
                }
            }

            if attempt < max_retries {
                sleep(interval).await;
            }
        }

        Err(ProcessError::NotReady(max_retries))
    }

    /// 启动进程并等待 API 就绪。
    ///
    /// 相当于 `start()` + `wait_ready(max_retries, interval)`。
    pub async fn start_and_wait(
        &self,
        max_retries: u32,
        interval: Duration,
    ) -> Result<u32, ProcessError> {
        let pid = self.start().await?;
        self.wait_ready(max_retries, interval).await?;
        Ok(pid)
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => {} // 已退出
                _ => {
                    if self.kill_on_drop {
                        warn!("MihomoManager dropped while process still running, killing...");
                        // Drop 中无法 await，直接调用 start_kill 发信号即可
                        let _ = child.start_kill();
                    } else {
                        warn!("MihomoManager dropped while process still running (kill_on_drop=false, leaving process alive)");
                    }
                }
            }
        }
    }
}

/// 终止一个子进程并等待其退出。
async fn kill_process(child: &mut Child) -> Result<(), ProcessError> {
    child.start_kill().map_err(|e| {
        error!("failed to kill process: {e}");
        ProcessError::Io(e)
    })?;
    // 异步等待进程彻底结束，回收资源
    let _ = child.wait().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用一个肯定不存在的路径测试 start 应该失败
    #[tokio::test]
    async fn start_binary_not_found() {
        let mgr = MihomoManager::new("./nonexistent_binary_12345", "./fake_config.yaml");
        let err = mgr.start().await.unwrap_err();
        assert!(matches!(err, ProcessError::BinaryNotFound(_)));
    }

    /// 没启动就 stop 应该报 NotRunning
    #[tokio::test]
    async fn stop_not_running() {
        let mgr = MihomoManager::new("./something", "./config.yaml");
        let err = mgr.stop().await.unwrap_err();
        assert!(matches!(err, ProcessError::NotRunning));
    }

    /// 初始状态应该是 Stopped
    #[tokio::test]
    async fn initial_status_stopped() {
        let mgr = MihomoManager::new("./something", "./config.yaml");
        assert_eq!(mgr.status().await, ProcessStatus::Stopped);
        assert!(!mgr.is_running().await);
    }

    #[tokio::test]
    async fn test_set_config_flag() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        mgr.set_config_flag("--config").await;
        let inner = mgr.inner.lock().await;
        assert_eq!(inner.config_flag, "--config");
    }

    #[tokio::test]
    async fn test_set_paths() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        mgr.set_binary_path("./new_bin").await;
        mgr.set_config_path("./new_cfg").await;
        assert_eq!(mgr.binary_path().await, PathBuf::from("./new_bin"));
        assert_eq!(mgr.config_path().await, PathBuf::from("./new_cfg"));
    }

    #[tokio::test]
    async fn test_set_home_dir() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        mgr.set_home_dir("/opt/mihomo").await;
        {
            let inner = mgr.inner.lock().await;
            assert_eq!(
                inner.home_dir.as_ref().unwrap(),
                &PathBuf::from("/opt/mihomo")
            );
        }
        mgr.clear_home_dir().await;
        {
            let inner = mgr.inner.lock().await;
            assert!(inner.home_dir.is_none());
        }
    }

    #[tokio::test]
    async fn test_set_ext_ctl_pipe() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        mgr.set_ext_ctl_pipe(r"\\.\pipe\my_mihomo").await;
        {
            let inner = mgr.inner.lock().await;
            assert_eq!(inner.ext_ctl_pipe.as_ref().unwrap(), r"\\.\pipe\my_mihomo");
        }
        mgr.clear_ext_ctl_pipe().await;
        {
            let inner = mgr.inner.lock().await;
            assert!(inner.ext_ctl_pipe.is_none());
        }
    }

    #[tokio::test]
    async fn test_set_secret() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        mgr.set_secret("my_secret").await;
        {
            let inner = mgr.inner.lock().await;
            assert_eq!(inner.secret.as_ref().unwrap(), "my_secret");
        }
        mgr.clear_secret().await;
        {
            let inner = mgr.inner.lock().await;
            assert!(inner.secret.is_none());
        }
    }

    #[tokio::test]
    async fn test_extra_args() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        mgr.add_extra_arg("-m").await;
        mgr.add_extra_arg("-ext-ctl").await;
        mgr.add_extra_arg("127.0.0.1:9090").await;
        {
            let inner = mgr.inner.lock().await;
            assert_eq!(inner.extra_args, vec!["-m", "-ext-ctl", "127.0.0.1:9090"]);
        }
        mgr.set_extra_args(vec!["-v".to_string()]).await;
        {
            let inner = mgr.inner.lock().await;
            assert_eq!(inner.extra_args, vec!["-v"]);
        }
        mgr.clear_extra_args().await;
        {
            let inner = mgr.inner.lock().await;
            assert!(inner.extra_args.is_empty());
        }
    }

    #[tokio::test]
    async fn test_kill_on_drop_flag() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        {
            let inner = mgr.inner.lock().await;
            assert!(inner.kill_on_drop); // default true
        }
        mgr.set_kill_on_drop(false).await;
        {
            let inner = mgr.inner.lock().await;
            assert!(!inner.kill_on_drop);
        }
    }

    #[tokio::test]
    async fn test_api_accessor() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        assert_eq!(mgr.api().pipe_name(), r"\\.\pipe\mihomo");
    }

    #[tokio::test]
    async fn test_with_transport() {
        let transport = PipeTransport::new()
            .with_pipe_name(r"\\.\pipe\custom_mihomo")
            .with_secret("test_secret");

        let mgr = MihomoManager::with_transport("./bin", "./cfg", transport);
        assert_eq!(mgr.api().pipe_name(), r"\\.\pipe\custom_mihomo");
    }

    /// wait_ready 在进程未运行时应立即返回 NotRunning
    #[tokio::test]
    async fn wait_ready_not_running() {
        let mgr = MihomoManager::new("./bin", "./cfg");
        let err = mgr
            .wait_ready(3, Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(matches!(err, ProcessError::NotRunning));
    }

    /// 用一个能正常启动的程序来测试完整的生命周期。
    #[tokio::test]
    async fn start_stop_real_process() {
        let tmp = std::env::temp_dir().join("mihomo_sdk_test_dummy.yaml");
        std::fs::write(&tmp, "dummy: true").unwrap();

        #[cfg(windows)]
        let mgr = {
            let m = MihomoManager::new("C:\\Windows\\System32\\ping.exe", &tmp);
            m.set_config_flag("-n").await;
            m
        };

        #[cfg(not(windows))]
        let mgr = {
            let m = MihomoManager::new("/bin/sleep", &tmp);
            m.set_config_flag("").await;
            m.set_config_path("2").await;
            m
        };

        // 先确认没跑
        assert_eq!(mgr.status().await, ProcessStatus::Stopped);

        // 启动
        let pid = mgr.start().await.unwrap();
        assert!(pid > 0);
        assert!(mgr.is_running().await);

        // 再启动应该报 AlreadyRunning
        let err = mgr.start().await.unwrap_err();
        assert!(matches!(err, ProcessError::AlreadyRunning(_)));

        // 停止
        mgr.stop().await.unwrap();
        assert_eq!(mgr.status().await, ProcessStatus::Stopped);

        // 清理
        let _ = std::fs::remove_file(&tmp);
    }

    #[tokio::test]
    async fn restart_not_running_starts() {
        let tmp = std::env::temp_dir().join("mihomo_sdk_test_restart.yaml");
        std::fs::write(&tmp, "dummy: true").unwrap();

        #[cfg(windows)]
        let mgr = {
            let m = MihomoManager::new("C:\\Windows\\System32\\ping.exe", &tmp);
            m.set_config_flag("-n").await;
            m
        };

        #[cfg(not(windows))]
        let mgr = {
            let m = MihomoManager::new("/bin/sleep", &tmp);
            m.set_config_flag("").await;
            m.set_config_path("2").await;
            m
        };

        // restart 在没运行时应该直接启动
        let pid = mgr.restart().await.unwrap();
        assert!(pid > 0);
        assert!(mgr.is_running().await);

        // 清理
        mgr.stop().await.unwrap();
        let _ = std::fs::remove_file(&tmp);
    }
}
