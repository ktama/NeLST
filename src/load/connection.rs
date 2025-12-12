//! コネクション負荷テストモジュール
//!
//! 大量のTCPコネクションを確立し、サーバのコネクション処理能力をテストする。

use crate::cli::load::ConnectionArgs;
use crate::common::error::Result;
use crate::common::output::create_progress_bar;
use crate::common::stats::{LatencyCollector, LoadTestResult, Timer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Semaphore};
use tracing::debug;

/// コネクション負荷テストを実行
pub async fn run(args: &ConnectionArgs) -> Result<LoadTestResult> {
    let timer = Timer::new();
    let target = args.target;
    let count = args.count;
    let concurrency = args.concurrency;
    let timeout = Duration::from_millis(args.timeout);
    let keep_alive = args.keep_alive;

    // 共有カウンター
    let success = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(Mutex::new(LatencyCollector::with_capacity(count)));

    // 同時接続数を制限するセマフォ
    let semaphore = Arc::new(Semaphore::new(concurrency));

    // プログレスバー
    let pb = create_progress_bar(count as u64, "Connections");

    // 接続を維持するためのベクター
    let connections: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));

    // タスクを生成
    let mut handles = Vec::new();
    for i in 0..count {
        let semaphore = semaphore.clone();
        let success = success.clone();
        let failed = failed.clone();
        let latencies = latencies.clone();
        let pb = pb.clone();
        let connections = connections.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let start = Instant::now();

            let result = tokio::time::timeout(timeout, TcpStream::connect(target)).await;

            match result {
                Ok(Ok(stream)) => {
                    let latency = start.elapsed();
                    success.fetch_add(1, Ordering::Relaxed);
                    latencies.lock().await.add_duration(latency);
                    debug!("Connection {} established in {:?}", i, latency);

                    if keep_alive {
                        connections.lock().await.push(stream);
                    }
                    // keep_alive が false の場合、stream はドロップされて接続が閉じる
                }
                Ok(Err(e)) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    debug!("Connection {} failed: {}", i, e);
                }
                Err(_) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    debug!("Connection {} timed out", i);
                }
            }

            pb.inc(1);
        });
        handles.push(handle);
    }

    // 全タスクの完了を待機
    for handle in handles {
        let _ = handle.await;
    }

    pb.finish_and_clear();

    // keep_alive の場合、接続を維持した状態で duration 秒待機
    if keep_alive && args.duration > 0 {
        let conn_count = connections.lock().await.len();
        println!(
            "Keeping {} connections alive for {} seconds...",
            conn_count, args.duration
        );
        tokio::time::sleep(Duration::from_secs(args.duration)).await;
    }

    // 結果を集計
    let elapsed = timer.elapsed_secs();
    let success_count = success.load(Ordering::Relaxed);
    let failed_count = failed.load(Ordering::Relaxed);

    let mut lat = latencies.lock().await;
    let latency_stats = lat.compute();

    Ok(LoadTestResult {
        target: target.to_string(),
        protocol: "tcp".to_string(),
        duration_secs: elapsed,
        total_requests: count as u64,
        successful_requests: success_count,
        failed_requests: failed_count,
        throughput_rps: if elapsed > 0.0 {
            count as f64 / elapsed
        } else {
            0.0
        },
        bytes_sent: 0,
        bytes_received: 0,
        latency: latency_stats,
    })
}
