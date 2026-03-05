//! Async `Stream` adapter that reads newline-delimited JSON objects from a
//! Windows Named Pipe connection.
//!
//! mihomo's streaming endpoints (`/logs`, `/traffic`, `/memory`,
//! `/connections` in WS-like mode) keep the HTTP connection open and
//! continuously write one JSON object per line (`\n`-delimited).
//!
//! [`PipeStream`] wraps the raw pipe client, parses the HTTP response
//! header once, and then yields deserialized `T` items as they arrive.

use std::io;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use pin_project_lite::pin_project;
use serde::de::DeserializeOwned;
use tokio::io::AsyncRead;
use tokio::io::ReadBuf;
use tokio::net::windows::named_pipe::NamedPipeClient;

use crate::ProcessError;

/// Internal read‐buffer size (bytes) for pipe reads.
const READ_BUF_SIZE: usize = 8192;

/// State machine for the stream lifecycle.
enum StreamState {
    /// We are still reading the HTTP response header.
    ReadingHeader,
    /// Header parsed; we are now reading body lines.
    ReadingBody,
    /// The stream has finished (pipe closed / error).
    Done,
}

pin_project! {
    /// An async [`Stream`] that reads newline-delimited JSON from a named pipe.
    ///
    /// Each call to `poll_next` drives the internal read loop:
    /// 1. On the very first read(s) it consumes the HTTP response header
    ///    (everything up to `\r\n\r\n`).
    /// 2. Once the header is consumed, every complete `\n`-terminated line
    ///    is deserialized as `T` and yielded.
    /// 3. When the pipe is closed (EOF) or an error occurs, the stream ends.
    ///
    /// # Extracting a `PipeStream`
    ///
    /// You don't construct this directly — use
    /// [`PipeTransport::stream_get`](super::transport::PipeTransport::stream_get)
    /// or one of the higher-level helpers on
    /// [`MihomoManager`](crate::MihomoManager).
    ///
    /// # Cancellation
    ///
    /// Dropping the `PipeStream` closes the underlying pipe handle, which is
    /// the cleanest way to cancel a streaming subscription.
    pub struct PipeStream<T> {
        #[pin]
        pipe: NamedPipeClient,
        buf: Vec<u8>,
        pending: Vec<T>,
        state: StreamState,
        http_status: u16,
        _marker: PhantomData<T>,
    }
}

impl<T> PipeStream<T> {
    /// Create a new `PipeStream` wrapping an already-connected pipe client.
    ///
    /// The HTTP request must have already been written to the pipe before
    /// this is called.
    pub(crate) fn new(pipe: NamedPipeClient) -> Self {
        Self {
            pipe,
            buf: Vec::with_capacity(READ_BUF_SIZE),
            pending: Vec::new(),
            state: StreamState::ReadingHeader,
            http_status: 0,
            _marker: PhantomData,
        }
    }

    /// Returns the HTTP status code from the response header.
    ///
    /// This is `0` until the header has been fully received and parsed
    /// (i.e. after the first item is yielded, or after the first
    /// `poll_next` that completes header parsing).
    pub fn http_status(&self) -> u16 {
        self.http_status
    }
}

impl<T: DeserializeOwned> Stream for PipeStream<T> {
    type Item = Result<T, ProcessError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.project();

        // 1. If we have pending items queued, yield one immediately.
        if !me.pending.is_empty() {
            // We stored them in order; pop from the front.
            let item = me.pending.remove(0);
            return Poll::Ready(Some(Ok(item)));
        }

        // 2. If we're done, return None.
        if matches!(me.state, StreamState::Done) {
            return Poll::Ready(None);
        }

        // 3. Try to read more data from the pipe.
        let mut tmp = [0u8; READ_BUF_SIZE];
        let mut read_buf = ReadBuf::new(&mut tmp);

        match me.pipe.poll_read(cx, &mut read_buf) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(e)) => {
                *me.state = StreamState::Done;
                // BrokenPipe is a normal "server closed" condition.
                if e.kind() == io::ErrorKind::BrokenPipe {
                    return Poll::Ready(None);
                }
                return Poll::Ready(Some(Err(ProcessError::Io(e))));
            }
            Poll::Ready(Ok(())) => {
                let filled = read_buf.filled();
                if filled.is_empty() {
                    // EOF
                    *me.state = StreamState::Done;

                    // There might be a trailing partial line in buf — try
                    // to parse it as one last item.
                    if !me.buf.is_empty() {
                        let leftover = std::mem::take(me.buf);
                        let text = String::from_utf8_lossy(&leftover);
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            match serde_json::from_str::<T>(trimmed) {
                                Ok(item) => return Poll::Ready(Some(Ok(item))),
                                Err(_) => { /* ignore trailing garbage */ }
                            }
                        }
                    }

                    return Poll::Ready(None);
                }

                me.buf.extend_from_slice(filled);
            }
        }

        // 4. If we haven't parsed the HTTP header yet, try to find
        //    the end-of-headers marker (`\r\n\r\n`).
        if matches!(me.state, StreamState::ReadingHeader) {
            if let Some(pos) = find_header_end(me.buf) {
                // Parse the status line / headers.
                let header_bytes = &me.buf[..pos];
                *me.http_status = parse_status_code(header_bytes);

                // Check for non-2xx status — read the rest as an error body.
                if *me.http_status >= 300 || *me.http_status == 0 {
                    // Drain header, keep whatever body we got so far.
                    let body_start = pos + 4; // skip `\r\n\r\n`
                    let body = if body_start < me.buf.len() {
                        String::from_utf8_lossy(&me.buf[body_start..]).into_owned()
                    } else {
                        String::new()
                    };
                    *me.state = StreamState::Done;
                    return Poll::Ready(Some(Err(ProcessError::Io(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "streaming request failed with HTTP {}: {}",
                            me.http_status,
                            body.trim()
                        ),
                    )))));
                }

                // Remove header from the buffer; keep body bytes.
                let body_start = pos + 4;
                let remaining = me.buf.split_off(body_start);
                me.buf.clear();
                *me.buf = remaining;
                *me.state = StreamState::ReadingBody;
            } else {
                // Header not fully received yet — wait for more data.
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }

        // 5. We're in ReadingBody. Extract complete lines and parse them.
        drain_lines(me.buf, me.pending);

        if !me.pending.is_empty() {
            let item = me.pending.remove(0);
            return Poll::Ready(Some(Ok(item)));
        }

        // No complete line yet — need more data.
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

// ── helpers ──────────────────────────────────────────────────────────

/// Find the byte offset of the `\r\n\r\n` sequence that marks the end of
/// the HTTP headers. Returns the position of the first `\r` of that
/// sequence.
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse the HTTP status code out of a raw header block.
/// Expects the very first line to be `HTTP/1.x <code> <reason>`.
fn parse_status_code(header_bytes: &[u8]) -> u16 {
    let line = match header_bytes.iter().position(|&b| b == b'\r') {
        Some(pos) => &header_bytes[..pos],
        None => header_bytes,
    };
    // e.g. "HTTP/1.1 200 OK"
    let text = std::str::from_utf8(line).unwrap_or("");
    text.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0)
}

/// Extract complete `\n`-terminated lines from `buf`, attempt to
/// deserialize each one, and push successful results into `out`.
///
/// Incomplete trailing data is left in `buf`.
fn drain_lines<T: DeserializeOwned>(buf: &mut Vec<u8>, out: &mut Vec<T>) {
    loop {
        let newline_pos = match buf.iter().position(|&b| b == b'\n') {
            Some(p) => p,
            None => break,
        };

        // Grab the line (without the `\n`).
        let line_bytes = &buf[..newline_pos];
        let text = String::from_utf8_lossy(line_bytes);
        let trimmed = text.trim();

        if !trimmed.is_empty() {
            if let Ok(item) = serde_json::from_str::<T>(trimmed) {
                out.push(item);
            }
            // Silently skip lines that fail to parse (e.g. chunked transfer
            // encoding hex lengths, empty keep-alive pings, etc.).
        }

        // Remove the line + newline from the buffer.
        // (drain is inclusive of newline_pos)
        buf.drain(..=newline_pos);
    }
}

// ── tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_header_end() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n{\"up\":1}";
        assert_eq!(find_header_end(data), Some(41));

        let no_end = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n";
        assert_eq!(find_header_end(no_end), None);
    }

    #[test]
    fn test_parse_status_code() {
        assert_eq!(parse_status_code(b"HTTP/1.1 200 OK\r\nHost: x"), 200);
        assert_eq!(parse_status_code(b"HTTP/1.1 404 Not Found"), 404);
        assert_eq!(parse_status_code(b"garbage"), 0);
    }

    #[test]
    fn test_drain_lines_traffic() {
        use crate::api::models::TrafficEntry;

        let mut buf = br#"{"up":100,"down":200}
{"up":300,"down":400,"upTotal":1000,"downTotal":2000}
partial"#
            .to_vec();

        let mut out: Vec<TrafficEntry> = Vec::new();
        drain_lines(&mut buf, &mut out);

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].up, 100);
        assert_eq!(out[0].down, 200);
        assert_eq!(out[0].up_total, 0); // missing → default
        assert_eq!(out[1].up, 300);
        assert_eq!(out[1].up_total, 1000);

        // "partial" should remain in the buffer.
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "partial");
    }

    #[test]
    fn test_drain_lines_logs() {
        use crate::api::models::LogEntry;

        let mut buf = br#"{"type":"info","payload":"hello world"}
{"type":"warning","payload":"something happened"}
"#
        .to_vec();

        let mut out: Vec<LogEntry> = Vec::new();
        drain_lines(&mut buf, &mut out);

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].level, "info");
        assert_eq!(out[0].payload, "hello world");
        assert_eq!(out[1].level, "warning");

        assert!(buf.is_empty());
    }

    #[test]
    fn test_drain_lines_memory() {
        use crate::api::models::MemoryEntry;

        let mut buf = br#"{"inuse":12345678}
{"inuse":23456789,"oslimit":0}
"#
        .to_vec();

        let mut out: Vec<MemoryEntry> = Vec::new();
        drain_lines(&mut buf, &mut out);

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].inuse, 12345678);
        assert_eq!(out[0].oslimit, 0);
        assert_eq!(out[1].inuse, 23456789);
    }

    #[test]
    fn test_drain_lines_empty_and_whitespace() {
        use crate::api::models::TrafficEntry;

        let mut buf = b"\n  \n{\"up\":1,\"down\":2}\n\n".to_vec();
        let mut out: Vec<TrafficEntry> = Vec::new();
        drain_lines(&mut buf, &mut out);

        // Only the valid JSON line should be parsed.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].up, 1);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_drain_lines_skips_unparseable() {
        use crate::api::models::TrafficEntry;

        let mut buf = b"not json\n{\"up\":5,\"down\":6}\n".to_vec();
        let mut out: Vec<TrafficEntry> = Vec::new();
        drain_lines(&mut buf, &mut out);

        // The invalid line is skipped, the valid one is parsed.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].up, 5);
    }
}
