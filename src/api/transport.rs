use std::io;

use log::debug;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::time::{timeout, Duration};

use crate::ProcessError;

/// 默认 named pipe 地址
const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\mihomo";

/// 读取响应的默认超时时间
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// 连接重试间隔
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(50);

/// 最大连接重试次数
const MAX_CONNECT_RETRIES: u32 = 3;

/// 通过 Windows Named Pipe 与 mihomo 进程通信的 HTTP 传输层。
///
/// mihomo 通过 `external-controller-pipe` 配置项暴露一个 named pipe，
/// 客户端连接后按 HTTP/1.1 协议收发请求/响应。
///
/// 每次请求建立一个新的 pipe 连接（短连接），发送完整的 HTTP 请求报文，
/// 读取完整的 HTTP 响应报文后关闭。
#[derive(Debug, Clone)]
pub struct PipeTransport {
    pipe_name: String,
    timeout: Duration,
    secret: Option<String>,
}

/// 解析后的 HTTP 响应
#[derive(Debug)]
pub struct HttpResponse {
    /// HTTP 状态码，例如 200、204、404
    pub status: u16,
    /// 响应体（JSON 字符串或空）
    pub body: String,
}

impl PipeTransport {
    /// 用默认 pipe 地址 `\\.\pipe\mihomo` 创建传输层。
    pub fn new() -> Self {
        Self {
            pipe_name: DEFAULT_PIPE_NAME.to_string(),
            timeout: DEFAULT_TIMEOUT,
            secret: None,
        }
    }

    /// 指定自定义 pipe 名称。
    pub fn with_pipe_name(mut self, name: impl Into<String>) -> Self {
        self.pipe_name = name.into();
        self
    }

    /// 设置请求超时时间。
    pub fn with_timeout(mut self, dur: Duration) -> Self {
        self.timeout = dur;
        self
    }

    /// 设置 API 密钥（对应配置文件中的 `secret`）。
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// 获取当前 pipe 名称。
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }

    // ── 公开的 HTTP 方法 ──────────────────────────────────────────────

    /// 发送 GET 请求。
    pub async fn get(&self, path: &str) -> Result<HttpResponse, ProcessError> {
        self.request("GET", path, None).await
    }

    /// 发送 PUT 请求，附带 JSON body。
    pub async fn put(&self, path: &str, body: &str) -> Result<HttpResponse, ProcessError> {
        self.request("PUT", path, Some(body)).await
    }

    /// 发送 POST 请求，附带可选 JSON body。
    pub async fn post(&self, path: &str, body: Option<&str>) -> Result<HttpResponse, ProcessError> {
        self.request("POST", path, body).await
    }

    /// 发送 PATCH 请求，附带 JSON body。
    pub async fn patch(&self, path: &str, body: &str) -> Result<HttpResponse, ProcessError> {
        self.request("PATCH", path, Some(body)).await
    }

    /// 发送 DELETE 请求。
    pub async fn delete(&self, path: &str) -> Result<HttpResponse, ProcessError> {
        self.request("DELETE", path, None).await
    }

    // ── 内部实现 ─────────────────────────────────────────────────────

    /// 构建 HTTP/1.1 请求报文并通过 named pipe 收发。
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<HttpResponse, ProcessError> {
        let raw_request = self.build_request(method, path, body);

        debug!("pipe request: {} {}", method, path);

        let raw_response = timeout(self.timeout, self.send_raw(&raw_request))
            .await
            .map_err(|_| {
                ProcessError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("request timed out: {} {}", method, path),
                ))
            })??;

        let response = Self::parse_response(&raw_response)?;

        debug!(
            "pipe response: {} (body {} bytes)",
            response.status,
            response.body.len()
        );

        Ok(response)
    }

    /// 构建原始 HTTP/1.1 请求字节。
    fn build_request(&self, method: &str, path: &str, body: Option<&str>) -> Vec<u8> {
        let mut headers = String::new();

        headers.push_str(&format!("{} {} HTTP/1.1\r\n", method, path));
        headers.push_str("Host: mihomo\r\n");
        headers.push_str("Connection: close\r\n");

        if let Some(ref secret) = self.secret {
            headers.push_str(&format!("Authorization: Bearer {}\r\n", secret));
        }

        match body {
            Some(b) => {
                headers.push_str("Content-Type: application/json\r\n");
                headers.push_str(&format!("Content-Length: {}\r\n", b.len()));
                headers.push_str("\r\n");
                let mut buf = headers.into_bytes();
                buf.extend_from_slice(b.as_bytes());
                buf
            }
            None => {
                headers.push_str("Content-Length: 0\r\n");
                headers.push_str("\r\n");
                headers.into_bytes()
            }
        }
    }

    /// 连接 named pipe 并发送/接收原始字节。
    async fn send_raw(&self, request: &[u8]) -> Result<Vec<u8>, ProcessError> {
        let mut pipe = self.connect_pipe().await?;

        // 写入请求
        pipe.write_all(request).await?;

        // 读取响应 —— 持续读直到 EOF（服务端关闭连接）
        let mut response = Vec::with_capacity(4096);
        let mut buf = [0u8; 4096];
        loop {
            match pipe.read(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => response.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => break,
                Err(e) => return Err(ProcessError::Io(e)),
            }
        }

        Ok(response)
    }

    /// 连接到 named pipe，带重试。
    async fn connect_pipe(
        &self,
    ) -> Result<tokio::net::windows::named_pipe::NamedPipeClient, ProcessError> {
        let mut last_err = None;

        for attempt in 0..MAX_CONNECT_RETRIES {
            match ClientOptions::new().open(&self.pipe_name) {
                Ok(client) => return Ok(client),
                Err(e) => {
                    debug!(
                        "pipe connect attempt {} failed: {} ({})",
                        attempt + 1,
                        e,
                        self.pipe_name
                    );
                    last_err = Some(e);
                    if attempt + 1 < MAX_CONNECT_RETRIES {
                        tokio::time::sleep(CONNECT_RETRY_DELAY).await;
                    }
                }
            }
        }

        Err(ProcessError::Io(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("failed to connect to pipe: {}", self.pipe_name),
            )
        })))
    }

    /// 用 httparse 解析原始 HTTP 响应字节。
    fn parse_response(raw: &[u8]) -> Result<HttpResponse, ProcessError> {
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut resp = httparse::Response::new(&mut headers);

        let body_offset = match resp.parse(raw) {
            Ok(httparse::Status::Complete(offset)) => offset,
            Ok(httparse::Status::Partial) => {
                // 尽力解析，把已有数据当作完整响应
                return Ok(HttpResponse {
                    status: resp.code.unwrap_or(0),
                    body: String::new(),
                });
            }
            Err(e) => {
                return Err(ProcessError::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to parse HTTP response: {}", e),
                )));
            }
        };

        let status = resp.code.unwrap_or(0);
        let body_bytes = &raw[body_offset..];
        let body = String::from_utf8_lossy(body_bytes).into_owned();

        Ok(HttpResponse { status, body })
    }
}

impl Default for PipeTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_get_request() {
        let transport = PipeTransport::new();
        let raw = transport.build_request("GET", "/version", None);
        let text = String::from_utf8(raw).unwrap();

        assert!(text.starts_with("GET /version HTTP/1.1\r\n"));
        assert!(text.contains("Host: mihomo\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.contains("Content-Length: 0\r\n"));
        assert!(text.ends_with("\r\n\r\n"));
        assert!(!text.contains("Authorization"));
    }

    #[test]
    fn test_build_put_request_with_body() {
        let transport = PipeTransport::new();
        let body = r#"{"mixed-port": 7890}"#;
        let raw = transport.build_request("PUT", "/configs?force=true", Some(body));
        let text = String::from_utf8(raw).unwrap();

        assert!(text.starts_with("PUT /configs?force=true HTTP/1.1\r\n"));
        assert!(text.contains("Content-Type: application/json\r\n"));
        assert!(text.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert!(text.ends_with(body));
    }

    #[test]
    fn test_build_request_with_secret() {
        let transport = PipeTransport::new().with_secret("my_token_123");
        let raw = transport.build_request("GET", "/configs", None);
        let text = String::from_utf8(raw).unwrap();

        assert!(text.contains("Authorization: Bearer my_token_123\r\n"));
    }

    #[test]
    fn test_parse_response_ok() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"version\":\"1.0\"}";
        let resp = PipeTransport::parse_response(raw).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, r#"{"version":"1.0"}"#);
    }

    #[test]
    fn test_parse_response_no_body() {
        let raw = b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
        let resp = PipeTransport::parse_response(raw).unwrap();
        assert_eq!(resp.status, 204);
        assert!(resp.body.is_empty());
    }

    #[test]
    fn test_parse_response_invalid() {
        let raw = b"not http at all";
        let result = PipeTransport::parse_response(raw);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_pipe_name() {
        let transport = PipeTransport::new();
        assert_eq!(transport.pipe_name(), r"\\.\pipe\mihomo");
    }

    #[test]
    fn test_custom_pipe_name() {
        let transport = PipeTransport::new().with_pipe_name(r"\\.\pipe\custom");
        assert_eq!(transport.pipe_name(), r"\\.\pipe\custom");
    }

    #[test]
    fn test_custom_timeout() {
        let transport = PipeTransport::new().with_timeout(Duration::from_secs(30));
        assert_eq!(transport.timeout, Duration::from_secs(30));
    }
}
