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

/// TCPリクエストを実行
async fn run_tcp_request(
    target: std::net::SocketAddr,
    mode: &TrafficMode,
    size: usize,
) -> Result<(usize, usize)> {
    let mut stream = TcpStream::connect(target).await.map_err(|e| {
        NelstError::connection_with_source(format!("Failed to connect to {}", target), e)
    })?;

    let data = vec![0x41u8; size];
    let mut sent = 0;
    let mut received = 0;

    match mode {
        TrafficMode::Send => {
            stream.write_all(&data).await?;
            sent = size;
        }
        TrafficMode::Echo => {
            stream.write_all(&data).await?;
            sent = size;
            let mut buf = vec![0u8; size];
            let n = stream.read(&mut buf).await?;
            received = n;
        }
        TrafficMode::Recv => {
            let mut buf = vec![0u8; size];
            let n = stream.read(&mut buf).await?;
            received = n;
        }
    }

    Ok((sent, received))
}

/// UDPリクエストを実行
async fn run_udp_request(
    target: std::net::SocketAddr,
    mode: &TrafficMode,
    size: usize,
) -> Result<(usize, usize)> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(target).await?;

    let data = vec![0x41u8; size.min(65507)];
    let mut sent = 0;
    let mut received = 0;

    match mode {
        TrafficMode::Send => {
            socket.send(&data).await?;
            sent = data.len();
        }
        TrafficMode::Echo => {
            socket.send(&data).await?;
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
