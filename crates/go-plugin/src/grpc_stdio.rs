//! gRPC stdio streaming.
//!
//! Captures plugin subprocess stdout/stderr and streams it to the host
//! over a gRPC service. This allows the host to mirror plugin output.
//!
//! Mirrors Go's `grpc_stdio.go`.

use crate::grpc_server::pb;
use std::io::Write;
use std::pin::Pin;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

/// Server-side gRPC stdio service.
///
/// Captures the plugin's stdout and stderr via OS pipes and streams
/// them as `StdioData` messages to the host.
///
/// Mirrors Go's `grpcStdioServer`.
pub struct GRPCStdioServer {
    /// Merged channel: receives (channel, data) tuples from reader tasks.
    merged_rx: tokio::sync::Mutex<mpsc::Receiver<(pb::stdio_data::Channel, Vec<u8>)>>,
}

impl GRPCStdioServer {
    /// Create from async readers.
    ///
    /// Spawns background tasks to read from stdout and stderr,
    /// merging both into a single channel for streaming.
    pub fn from_readers<R1, R2>(stdout: R1, stderr: R2) -> Self
    where
        R1: tokio::io::AsyncRead + Send + Unpin + 'static,
        R2: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        let (merged_tx, merged_rx) = mpsc::channel(128);

        let stdout_tx = merged_tx.clone();
        tokio::spawn(async move {
            read_into_channel(stdout, pb::stdio_data::Channel::Stdout, stdout_tx).await;
        });

        let stderr_tx = merged_tx;
        tokio::spawn(async move {
            read_into_channel(stderr, pb::stdio_data::Channel::Stderr, stderr_tx).await;
        });

        Self {
            merged_rx: tokio::sync::Mutex::new(merged_rx),
        }
    }

    /// Create by capturing the current process's stdout/stderr via OS pipes.
    ///
    /// This replaces `os.Stdout` and `os.Stderr` with pipes, mirroring
    /// Go's `Serve()` behavior. All subsequent writes to stdout/stderr
    /// will be captured and streamed.
    #[cfg(unix)]
    pub fn capture_stdio() -> std::io::Result<Self> {
        use std::os::unix::io::FromRawFd;

        let (merged_tx, merged_rx) = mpsc::channel(128);

        // Capture stdout
        let (stdout_read, stdout_write) = os_pipe()?;
        // Redirect fd 1 (stdout) to the write end
        unsafe {
            libc::dup2(stdout_write, 1);
            libc::close(stdout_write);
        }
        let stdout_reader =
            tokio::io::BufReader::new(unsafe { tokio::fs::File::from_raw_fd(stdout_read) });
        let stdout_tx = merged_tx.clone();
        tokio::spawn(async move {
            read_into_channel(stdout_reader, pb::stdio_data::Channel::Stdout, stdout_tx).await;
        });

        // Capture stderr
        let (stderr_read, stderr_write) = os_pipe()?;
        unsafe {
            libc::dup2(stderr_write, 2);
            libc::close(stderr_write);
        }
        let stderr_reader =
            tokio::io::BufReader::new(unsafe { tokio::fs::File::from_raw_fd(stderr_read) });
        let stderr_tx = merged_tx;
        tokio::spawn(async move {
            read_into_channel(stderr_reader, pb::stdio_data::Channel::Stderr, stderr_tx).await;
        });

        Ok(Self {
            merged_rx: tokio::sync::Mutex::new(merged_rx),
        })
    }
}

/// Create an OS pipe, returning (read_fd, write_fd).
#[cfg(unix)]
fn os_pipe() -> std::io::Result<(i32, i32)> {
    let mut fds = [0i32; 2];
    let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((fds[0], fds[1]))
}

async fn read_into_channel<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    channel: pb::stdio_data::Channel,
    tx: mpsc::Sender<(pb::stdio_data::Channel, Vec<u8>)>,
) {
    let mut buf = vec![0u8; 1024];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                if tx.send((channel, buf[..n].to_vec())).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

use pb::grpc_stdio_server::GrpcStdio;

#[tonic::async_trait]
impl GrpcStdio for GRPCStdioServer {
    type StreamStdioStream = Pin<
        Box<dyn tokio_stream::Stream<Item = Result<pb::StdioData, tonic::Status>> + Send>,
    >;

    async fn stream_stdio(
        &self,
        _request: tonic::Request<pb::StdioStreamRequest>,
    ) -> Result<tonic::Response<Self::StreamStdioStream>, tonic::Status> {
        // Take ownership of the merged receiver.
        // This RPC should only be called once (as documented in the proto).
        let mut rx = self.merged_rx.lock().await;

        // Create a channel to forward data
        let (tx, stream_rx) = mpsc::channel(128);

        // We need to move the receiver out. Since it's behind a Mutex,
        // we swap it with a dummy channel receiver.
        let (_, dummy_rx) = mpsc::channel(1);
        let real_rx = std::mem::replace(&mut *rx, dummy_rx);
        drop(rx);

        tokio::spawn(async move {
            let mut real_rx = real_rx;
            while let Some((channel, data)) = real_rx.recv().await {
                let msg = pb::StdioData {
                    channel: channel as i32,
                    data,
                };
                if tx.send(Ok(msg)).await.is_err() {
                    break;
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(stream_rx);
        Ok(tonic::Response::new(Box::pin(stream)))
    }
}

/// Client-side gRPC stdio handler.
///
/// Connects to the plugin's stdio stream and writes received data
/// to the configured stdout/stderr writers.
///
/// Mirrors Go's `grpcStdioClient`.
pub struct GRPCStdioClient {
    stdout: Box<dyn std::io::Write + Send>,
    stderr: Box<dyn std::io::Write + Send>,
}

impl GRPCStdioClient {
    pub fn new(
        stdout: Box<dyn std::io::Write + Send>,
        stderr: Box<dyn std::io::Write + Send>,
    ) -> Self {
        Self { stdout, stderr }
    }

    /// Start receiving stdio data from the plugin.
    ///
    /// Handles `Unavailable` and `Unimplemented` gracefully (older plugins
    /// may not support stdio streaming).
    pub async fn run(mut self, channel: tonic::transport::Channel) {
        let mut client = pb::grpc_stdio_client::GrpcStdioClient::new(channel);

        let stream = match client.stream_stdio(pb::StdioStreamRequest {}).await {
            Ok(response) => response.into_inner(),
            Err(status) => {
                // Gracefully handle missing service (older plugins)
                match status.code() {
                    tonic::Code::Unavailable | tonic::Code::Unimplemented => {
                        log::debug!("Plugin does not support stdio streaming");
                    }
                    _ => {
                        log::warn!("Stdio stream error: {status}");
                    }
                }
                return;
            }
        };

        self.process_stream(stream).await;
    }

    async fn process_stream(&mut self, mut stream: tonic::Streaming<pb::StdioData>) {
        while let Ok(Some(data)) = stream.message().await {
            let channel = data.channel();
            match channel {
                pb::stdio_data::Channel::Stdout => {
                    let _ = self.stdout.write_all(&data.data);
                    let _ = self.stdout.flush();
                }
                pb::stdio_data::Channel::Stderr => {
                    let _ = self.stderr.write_all(&data.data);
                    let _ = self.stderr.flush();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_client_creation() {
        let _client = GRPCStdioClient::new(
            Box::new(std::io::sink()),
            Box::new(std::io::sink()),
        );
    }

    #[tokio::test]
    async fn stdio_server_from_readers() {
        let stdout = tokio::io::empty();
        let stderr = tokio::io::empty();
        let _server = GRPCStdioServer::from_readers(stdout, stderr);
    }
}
