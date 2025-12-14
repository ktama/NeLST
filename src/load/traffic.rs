//! トラフィック負荷テストモジュール
//!
//! ターゲットへ指定したデータサイズのパケットを送信し続ける。

use crate::cli::load::{Protocol, TrafficArgs, TrafficMode};
use crate::common::error::{NelstError, Result};
use crate::common::output::create_duration_progress_bar;
use crate::common::stats::{LatencyCollector, LoadTestResult, Timer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tracing::debug;

/// トラフィック負荷テストを実行
pub async fn run(args: &TrafficArgs) -> Result<LoadTestResult> {
    let test = TrafficTest::new(args);
    test.run().await
}

/// トラフィック負荷テストの実行コンテキスト
pub struct TrafficTest {
    target: std::net::SocketAddr,
    protocol: Protocol,
    mode: TrafficMode,
    size: usize,
    duration_secs: u64,
    concurrency: usize,
    rate: Option<u64>,
}

impl TrafficTest {
    /// 新しいテストを作成
    pub fn new(args: &TrafficArgs) -> Self {
        Self {
            target: args.target,
            protocol: args.protocol.clone(),
            mode: args.mode.clone(),
            size: args.size,
            duration_secs: args.duration,
            concurrency: args.concurrency,
            rate: args.rate,
        }
    }

    /// テストを実行
    pub async fn run(&self) -> Result<LoadTestResult> {
        let timer = Timer::new();
        let running = Arc::new(AtomicBool::new(true));

        // 共有カウンター
        let total = Arc::new(AtomicU64::new(0));
        let success = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));
        let bytes_sent = Arc::new(AtomicU64::new(0));
        let bytes_received = Arc::new(AtomicU64::new(0));
        let latencies = Arc::new(Mutex::new(LatencyCollector::new()));

        // プログレスバー
        let pb = create_duration_progress_bar(self.duration_secs);

        // ワーカータスクを起動
        let mut handles = Vec::new();
        for worker_id in 0..self.concurrency {
            let target = self.target;
            let protocol = self.protocol.clone();
            let mode = self.mode.clone();
            let size = self.size;
            let running = running.clone();
            let total = total.clone();
            let success = success.clone();
            let failed = failed.clone();
            let bytes_sent = bytes_sent.clone();
            let bytes_received = bytes_received.clone();
            let latencies = latencies.clone();
            let rate = self.rate;
            let concurrency = self.concurrency;

            let handle = tokio::spawn(async move {
                let delay =
                    rate.map(|r| Duration::from_secs_f64(1.0 / r as f64 * concurrency as f64));

                while running.load(Ordering::Relaxed) {
                    let start = Instant::now();
                    let result = match protocol {
                        Protocol::Tcp => run_tcp_request(target, &mode, size).await,
                        Protocol::Udp => run_udp_request(target, &mode, size).await,
                    };

                    total.fetch_add(1, Ordering::Relaxed);

                    match result {
                        Ok((sent, received)) => {
                            success.fetch_add(1, Ordering::Relaxed);
                            bytes_sent.fetch_add(sent as u64, Ordering::Relaxed);
                            bytes_received.fetch_add(received as u64, Ordering::Relaxed);
                            let latency = start.elapsed();
                            latencies.lock().await.add_duration(latency);
                        }
                        Err(e) => {
                            failed.fetch_add(1, Ordering::Relaxed);
                            debug!("Worker {} error: {}", worker_id, e);
                        }
                    }

                    // レート制限
                    if let Some(d) = delay {
                        let elapsed = start.elapsed();
                        if elapsed < d {
                            tokio::time::sleep(d - elapsed).await;
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // 時間経過を監視
        let duration = Duration::from_secs(self.duration_secs);
        let start = Instant::now();
        while start.elapsed() < duration {
            tokio::time::sleep(Duration::from_secs(1)).await;
            pb.inc(1);
        }

        // 停止シグナル
        running.store(false, Ordering::Relaxed);
        pb.finish_and_clear();

        // ワーカー終了を待機
        for handle in handles {
            let _ = handle.await;
        }

        // 結果を集計
        let elapsed = timer.elapsed_secs();
        let total_count = total.load(Ordering::Relaxed);
        let success_count = success.load(Ordering::Relaxed);
        let failed_count = failed.load(Ordering::Relaxed);
        let sent = bytes_sent.load(Ordering::Relaxed);
        let received = bytes_received.load(Ordering::Relaxed);

        let mut lat = latencies.lock().await;
        let latency_stats = lat.compute();

        Ok(LoadTestResult {
            target: self.target.to_string(),
            protocol: format!("{:?}", self.protocol).to_lowercase(),
            duration_secs: elapsed,
            total_requests: total_count,
            successful_requests: success_count,
            failed_requests: failed_count,
            throughput_rps: if elapsed > 0.0 {
                total_count as f64 / elapsed
            } else {
                0.0
            },
            bytes_sent: sent,
            bytes_received: received,
            latency: latency_stats,
        })
    }
}

/// 送信データを生成（Box::leak で静的バッファを作成し再利用）
///
/// # Arguments
/// * `size` - 要求するデータサイズ（バイト）
///
/// # Returns
/// 指定サイズ（最大64KB）の静的スライス参照
fn get_send_data(size: usize) -> &'static [u8] {
    use std::sync::OnceLock;
    static BUFFER: OnceLock<Box<[u8]>> = OnceLock::new();

    // サイズ0の場合は空スライスを返す
    if size == 0 {
        return &[];
    }

    // 最大64KBの固定バッファを一度だけ確保
    let buf = BUFFER.get_or_init(|| vec![0x41u8; 65536].into_boxed_slice());

    &buf[..size.min(buf.len())]
}

/// TCPリクエストを実行（最適化版）
async fn run_tcp_request(
    target: std::net::SocketAddr,
    mode: &TrafficMode,
    size: usize,
) -> Result<(usize, usize)> {
    let mut stream = TcpStream::connect(target).await.map_err(|e| {
        NelstError::connection_with_source(format!("Failed to connect to {}", target), e)
    })?;

    let mut sent = 0;
    let mut received = 0;

    match mode {
        TrafficMode::Send => {
            // 静的バッファを再利用
            let data = get_send_data(size);
            stream.write_all(data).await?;
            sent = data.len();
        }
        TrafficMode::Echo => {
            let data = get_send_data(size);
            stream.write_all(data).await?;
            sent = data.len();
            // 受信バッファは毎回必要だが、サイズを制限
            let mut buf = vec![0u8; size.min(65536)];
            let n = stream.read(&mut buf).await?;
            received = n;
        }
        TrafficMode::Recv => {
            let mut buf = vec![0u8; size.min(65536)];
            let n = stream.read(&mut buf).await?;
            received = n;
        }
    }

    Ok((sent, received))
}

/// UDPリクエストを実行
///
/// # Arguments
/// * `target` - 接続先のソケットアドレス
/// * `mode` - 送受信モード
/// * `size` - 送信データサイズ
async fn run_udp_request(
    target: std::net::SocketAddr,
    mode: &TrafficMode,
    size: usize,
) -> Result<(usize, usize)> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(target).await?;

    // 静的バッファを再利用（UDPの最大ペイロードサイズに制限）
    let data = get_send_data(size.min(65507));
    let mut sent = 0;
    let mut received = 0;

    match mode {
        TrafficMode::Send => {
            socket.send(data).await?;
            sent = data.len();
        }
        TrafficMode::Echo => {
            socket.send(data).await?;
            sent = data.len();
            let mut buf = vec![0u8; 65535];
            // タイムアウト付きで受信
            match tokio::time::timeout(Duration::from_secs(1), socket.recv(&mut buf)).await {
                Ok(Ok(n)) => received = n,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => return Err(NelstError::timeout("UDP receive timeout")),
            }
        }
        TrafficMode::Recv => {
            // 最初にトリガーパケットを送信
            socket.send(&[0u8]).await?;
            let mut buf = vec![0u8; 65535];
            match tokio::time::timeout(Duration::from_secs(1), socket.recv(&mut buf)).await {
                Ok(Ok(n)) => received = n,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => return Err(NelstError::timeout("UDP receive timeout")),
            }
        }
    }

    Ok((sent, received))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn create_test_args(
        target: SocketAddr,
        protocol: Protocol,
        mode: TrafficMode,
        size: usize,
    ) -> TrafficArgs {
        TrafficArgs {
            target,
            protocol,
            mode,
            size,
            duration: 1,
            concurrency: 1,
            rate: None,
            output: None,
        }
    }

    #[test]
    fn test_get_send_data_zero_size() {
        let data = get_send_data(0);
        assert!(data.is_empty());
    }

    #[test]
    fn test_get_send_data_normal_size() {
        let data = get_send_data(100);
        assert_eq!(data.len(), 100);
        assert!(data.iter().all(|&b| b == 0x41));
    }

    #[test]
    fn test_get_send_data_max_size() {
        let data = get_send_data(65536);
        assert_eq!(data.len(), 65536);
    }

    #[test]
    fn test_get_send_data_over_max_size() {
        let data = get_send_data(100000);
        assert_eq!(data.len(), 65536); // 最大サイズにクランプ
    }

    #[test]
    fn test_traffic_test_new() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let args = create_test_args(addr, Protocol::Tcp, TrafficMode::Send, 1024);
        let test = TrafficTest::new(&args);

        assert_eq!(test.target, addr);
        assert_eq!(test.size, 1024);
        assert_eq!(test.duration_secs, 1);
        assert_eq!(test.concurrency, 1);
        assert!(test.rate.is_none());
    }

    #[test]
    fn test_traffic_mode_variants() {
        let modes = [TrafficMode::Send, TrafficMode::Echo, TrafficMode::Recv];
        assert_eq!(modes.len(), 3);
    }

    #[test]
    fn test_protocol_variants() {
        let protocols = [Protocol::Tcp, Protocol::Udp];
        assert_eq!(protocols.len(), 2);
    }

    #[tokio::test]
    async fn test_run_tcp_request_unreachable() {
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let result = run_tcp_request(addr, &TrafficMode::Send, 100).await;
        // 接続できないアドレスなのでエラー
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_udp_request_send_mode() {
        // UDPは接続不要なので送信自体は成功する可能性がある
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let result = run_udp_request(addr, &TrafficMode::Send, 100).await;
        // 送信は成功するはず（UDPは非接続型）
        assert!(result.is_ok());
        let (sent, received) = result.unwrap();
        assert_eq!(sent, 100);
        assert_eq!(received, 0);
    }

    #[test]
    fn test_traffic_args_with_rate() {
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let args = TrafficArgs {
            target: addr,
            protocol: Protocol::Tcp,
            mode: TrafficMode::Send,
            size: 1024,
            duration: 10,
            concurrency: 4,
            rate: Some(1000),
            output: None,
        };
        let test = TrafficTest::new(&args);
        assert_eq!(test.rate, Some(1000));
        assert_eq!(test.concurrency, 4);
    }
}
