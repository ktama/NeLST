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

/// バッチサイズ（一度に起動するタスク数）
const BATCH_SIZE: usize = 100;

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
    // ワーカーごとのレイテンシコレクターを使用して競合を削減
    let latencies = Arc::new(Mutex::new(LatencyCollector::with_capacity(count)));

    // 同時接続数を制限するセマフォ
    let semaphore = Arc::new(Semaphore::new(concurrency));

    // プログレスバー
    let pb = create_progress_bar(count as u64, "Connections");

    // 接続を維持するためのベクター（事前に容量確保）
    let connections: Arc<Mutex<Vec<TcpStream>>> =
        Arc::new(Mutex::new(Vec::with_capacity(if keep_alive {
            count
        } else {
            0
        })));

    // バッチ処理でタスクを生成（大量タスク生成のオーバーヘッド削減）
    let mut handles = Vec::with_capacity(count);

    for batch_start in (0..count).step_by(BATCH_SIZE) {
        let batch_end = (batch_start + BATCH_SIZE).min(count);

        for i in batch_start..batch_end {
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

        // バッチ間で短い休息を入れてCPU負荷を分散
        if batch_end < count {
            tokio::task::yield_now().await;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn create_test_args(target: SocketAddr, count: usize, concurrency: usize) -> ConnectionArgs {
        ConnectionArgs {
            target,
            count,
            concurrency,
            timeout: 1000,
            keep_alive: false,
            duration: 0,
            output: None,
        }
    }

    #[test]
    fn test_batch_size_constant() {
        // 定数のアサーションはconstブロックでコンパイル時に検証
        const _: () = assert!(BATCH_SIZE > 0, "BATCH_SIZE should be positive");
        const _: () = assert!(
            BATCH_SIZE <= 1000,
            "BATCH_SIZE should not be too large to avoid memory issues"
        );
        // テストが実行されることを確認
        assert_eq!(BATCH_SIZE, 100);
    }

    #[tokio::test]
    async fn test_run_with_zero_count() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let args = create_test_args(addr, 0, 10);
        let result = run(&args).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.total_requests, 0);
        assert_eq!(result.successful_requests, 0);
        assert_eq!(result.failed_requests, 0);
    }

    #[tokio::test]
    async fn test_run_with_unreachable_target() {
        // 接続できないアドレスを使用
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let args = create_test_args(addr, 3, 1);
        let result = run(&args).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.total_requests, 3);
        // 接続拒否またはタイムアウトで失敗するはず
        assert_eq!(result.failed_requests, 3);
    }

    #[tokio::test]
    async fn test_run_result_structure() {
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let args = create_test_args(addr, 1, 1);
        let result = run(&args).await.unwrap();

        assert_eq!(result.protocol, "tcp");
        assert!(result.duration_secs >= 0.0);
        assert_eq!(result.bytes_sent, 0);
        assert_eq!(result.bytes_received, 0);
    }

    #[tokio::test]
    async fn test_concurrency_higher_than_count() {
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        // concurrency > count の場合でも正常動作
        let args = create_test_args(addr, 2, 100);
        let result = run(&args).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().total_requests, 2);
    }

    #[test]
    fn test_connection_args_fields() {
        let addr: SocketAddr = "192.168.1.1:8080".parse().unwrap();
        let args = ConnectionArgs {
            target: addr,
            count: 100,
            concurrency: 10,
            timeout: 5000,
            keep_alive: true,
            duration: 30,
            output: Some("output.json".to_string()),
        };

        assert_eq!(args.target.port(), 8080);
        assert_eq!(args.count, 100);
        assert_eq!(args.concurrency, 10);
        assert_eq!(args.timeout, 5000);
        assert!(args.keep_alive);
        assert_eq!(args.duration, 30);
    }
}
