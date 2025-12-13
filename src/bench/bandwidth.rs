//! 帯域幅測定実装
//!
//! ネットワーク帯域幅を測定する（iperf3ライク）

use crate::cli::bench::{BandwidthArgs, BandwidthDirection};
use crate::common::error::NelstError;
use serde::Serialize;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

/// 帯域幅測定結果
#[derive(Debug, Clone, Serialize)]
pub struct BandwidthResult {
    /// モード（client/server）
    pub mode: String,
    /// ターゲット（クライアント時）
    pub target: Option<String>,
    /// バインドアドレス（サーバ時）
    pub bind: Option<String>,
    /// 測定時間（秒）
    pub duration_secs: u64,
    /// アップロード結果
    pub upload: Option<DirectionResult>,
    /// ダウンロード結果
    pub download: Option<DirectionResult>,
}

/// 方向別測定結果
#[derive(Debug, Clone, Serialize)]
pub struct DirectionResult {
    /// 転送バイト数
    pub bytes_transferred: u64,
    /// 平均帯域幅（Mbps）
    pub bandwidth_mbps: f64,
    /// ピーク帯域幅（Mbps）
    pub peak_mbps: f64,
    /// ジッター（ミリ秒）
    pub jitter_ms: f64,
    /// 各秒の帯域幅
    pub per_second_mbps: Vec<f64>,
}

/// 帯域幅サーバを実行
async fn run_server(bind: SocketAddr) -> Result<BandwidthResult, NelstError> {
    let listener = TcpListener::bind(bind).await.map_err(|e| {
        NelstError::connection(format!("Failed to bind to {}: {}", bind, e))
    })?;

    info!("Bandwidth server listening on {}", bind);

    loop {
        let (mut socket, peer) = listener.accept().await.map_err(|e| {
            NelstError::connection(format!("Accept failed: {}", e))
        })?;

        info!("Connection from {}", peer);

        // コントロールメッセージを受信
        let mut cmd_buf = [0u8; 1];
        match socket.read_exact(&mut cmd_buf).await {
            Ok(_) => {
                let cmd = cmd_buf[0];
                match cmd {
                    b'U' => {
                        // アップロードテスト（クライアントからの受信）
                        info!("Starting upload test (receiving from client)");
                        let mut total_bytes: u64 = 0;
                        let mut buf = vec![0u8; 131072];
                        let start = Instant::now();

                        loop {
                            match socket.read(&mut buf).await {
                                Ok(0) => break, // 接続終了
                                Ok(n) => {
                                    total_bytes += n as u64;
                                }
                                Err(_) => break,
                            }
                            if start.elapsed() > Duration::from_secs(60) {
                                break;
                            }
                        }

                        let elapsed = start.elapsed().as_secs_f64();
                        let mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
                        info!(
                            "Upload test completed: {} bytes in {:.2}s ({:.2} Mbps)",
                            total_bytes, elapsed, mbps
                        );
                    }
                    b'D' => {
                        // ダウンロードテスト（クライアントへの送信）
                        info!("Starting download test (sending to client)");
                        let data = vec![0xABu8; 131072];
                        let start = Instant::now();
                        let mut total_bytes: u64 = 0;

                        while start.elapsed() < Duration::from_secs(10) {
                            match socket.write_all(&data).await {
                                Ok(_) => {
                                    total_bytes += data.len() as u64;
                                }
                                Err(_) => break,
                            }
                        }

                        let elapsed = start.elapsed().as_secs_f64();
                        let mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
                        info!(
                            "Download test completed: {} bytes in {:.2}s ({:.2} Mbps)",
                            total_bytes, elapsed, mbps
                        );
                    }
                    _ => {
                        warn!("Unknown command: {}", cmd);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read command: {}", e);
            }
        }
    }
}

/// アップロードテストを実行
async fn run_upload_test(
    stream: &mut TcpStream,
    duration: Duration,
    block_size: usize,
) -> Result<DirectionResult, NelstError> {
    // コマンド送信
    stream.write_all(b"U").await.map_err(|e| {
        NelstError::connection(format!("Failed to send command: {}", e))
    })?;

    let data = vec![0xCDu8; block_size];
    let start = Instant::now();
    let mut total_bytes: u64 = 0;
    let mut per_second_bytes: Vec<u64> = Vec::new();
    let mut last_second_bytes: u64 = 0;
    let mut last_second = 0u64;

    while start.elapsed() < duration {
        match stream.write_all(&data).await {
            Ok(_) => {
                total_bytes += data.len() as u64;
            }
            Err(e) => {
                warn!("Write error: {}", e);
                break;
            }
        }

        // 秒ごとの統計
        let current_second = start.elapsed().as_secs();
        if current_second > last_second {
            per_second_bytes.push(total_bytes - last_second_bytes);
            last_second_bytes = total_bytes;
            last_second = current_second;
        }
    }

    // 残りを追加
    if total_bytes > last_second_bytes {
        per_second_bytes.push(total_bytes - last_second_bytes);
    }

    let elapsed = start.elapsed().as_secs_f64();
    let per_second_mbps: Vec<f64> = per_second_bytes
        .iter()
        .map(|b| (*b as f64 * 8.0) / 1_000_000.0)
        .collect();

    let bandwidth_mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
    let peak_mbps = per_second_mbps.iter().cloned().fold(0.0, f64::max);

    // ジッター計算（帯域幅の変動の標準偏差）
    let jitter_ms = if per_second_mbps.len() > 1 {
        let mean: f64 = per_second_mbps.iter().sum::<f64>() / per_second_mbps.len() as f64;
        let variance: f64 = per_second_mbps.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / per_second_mbps.len() as f64;
        variance.sqrt()
    } else {
        0.0
    };

    Ok(DirectionResult {
        bytes_transferred: total_bytes,
        bandwidth_mbps,
        peak_mbps,
        jitter_ms,
        per_second_mbps,
    })
}

/// ダウンロードテストを実行
async fn run_download_test(
    stream: &mut TcpStream,
    duration: Duration,
    block_size: usize,
) -> Result<DirectionResult, NelstError> {
    // コマンド送信
    stream.write_all(b"D").await.map_err(|e| {
        NelstError::connection(format!("Failed to send command: {}", e))
    })?;

    let mut buf = vec![0u8; block_size];
    let start = Instant::now();
    let mut total_bytes: u64 = 0;
    let mut per_second_bytes: Vec<u64> = Vec::new();
    let mut last_second_bytes: u64 = 0;
    let mut last_second = 0u64;

    while start.elapsed() < duration {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                total_bytes += n as u64;
            }
            Err(e) => {
                warn!("Read error: {}", e);
                break;
            }
        }

        let current_second = start.elapsed().as_secs();
        if current_second > last_second {
            per_second_bytes.push(total_bytes - last_second_bytes);
            last_second_bytes = total_bytes;
            last_second = current_second;
        }
    }

    if total_bytes > last_second_bytes {
        per_second_bytes.push(total_bytes - last_second_bytes);
    }

    let elapsed = start.elapsed().as_secs_f64();
    let per_second_mbps: Vec<f64> = per_second_bytes
        .iter()
        .map(|b| (*b as f64 * 8.0) / 1_000_000.0)
        .collect();

    let bandwidth_mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
    let peak_mbps = per_second_mbps.iter().cloned().fold(0.0, f64::max);

    let jitter_ms = if per_second_mbps.len() > 1 {
        let mean: f64 = per_second_mbps.iter().sum::<f64>() / per_second_mbps.len() as f64;
        let variance: f64 = per_second_mbps.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / per_second_mbps.len() as f64;
        variance.sqrt()
    } else {
        0.0
    };

    Ok(DirectionResult {
        bytes_transferred: total_bytes,
        bandwidth_mbps,
        peak_mbps,
        jitter_ms,
        per_second_mbps,
    })
}

/// 帯域幅クライアントを実行
async fn run_client(args: &BandwidthArgs) -> Result<BandwidthResult, NelstError> {
    let target = args.target.ok_or_else(|| {
        NelstError::config("Target address is required in client mode".to_string())
    })?;

    info!("Connecting to bandwidth server at {}", target);

    let duration = Duration::from_secs(args.duration);
    let mut upload_result = None;
    let mut download_result = None;

    // アップロードテスト
    if matches!(args.direction, BandwidthDirection::Up | BandwidthDirection::Both) {
        info!("Starting upload test...");
        let mut stream = TcpStream::connect(target).await.map_err(|e| {
            NelstError::connection(format!("Failed to connect to {}: {}", target, e))
        })?;

        let result = run_upload_test(&mut stream, duration, args.block_size).await?;
        info!(
            "Upload: {:.2} Mbps (peak: {:.2} Mbps)",
            result.bandwidth_mbps, result.peak_mbps
        );
        upload_result = Some(result);
    }

    // ダウンロードテスト
    if matches!(args.direction, BandwidthDirection::Down | BandwidthDirection::Both) {
        info!("Starting download test...");
        let mut stream = TcpStream::connect(target).await.map_err(|e| {
            NelstError::connection(format!("Failed to connect to {}: {}", target, e))
        })?;

        let result = run_download_test(&mut stream, duration, args.block_size).await?;
        info!(
            "Download: {:.2} Mbps (peak: {:.2} Mbps)",
            result.bandwidth_mbps, result.peak_mbps
        );
        download_result = Some(result);
    }

    Ok(BandwidthResult {
        mode: "client".to_string(),
        target: Some(target.to_string()),
        bind: None,
        duration_secs: args.duration,
        upload: upload_result,
        download: download_result,
    })
}

/// 帯域幅測定を実行
pub async fn run(args: &BandwidthArgs) -> Result<BandwidthResult, NelstError> {
    if args.server {
        run_server(args.bind).await
    } else {
        run_client(args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_result() {
        let result = DirectionResult {
            bytes_transferred: 10_000_000,
            bandwidth_mbps: 80.0,
            peak_mbps: 95.0,
            jitter_ms: 2.5,
            per_second_mbps: vec![75.0, 80.0, 85.0, 90.0, 70.0],
        };
        assert_eq!(result.bytes_transferred, 10_000_000);
        assert!(result.bandwidth_mbps > 0.0);
    }

    #[test]
    fn test_direction_result_zero() {
        let result = DirectionResult {
            bytes_transferred: 0,
            bandwidth_mbps: 0.0,
            peak_mbps: 0.0,
            jitter_ms: 0.0,
            per_second_mbps: vec![],
        };
        assert_eq!(result.bytes_transferred, 0);
        assert!(result.per_second_mbps.is_empty());
    }

    #[test]
    fn test_bandwidth_result() {
        let result = BandwidthResult {
            mode: "client".to_string(),
            target: Some("127.0.0.1:5201".to_string()),
            bind: None,
            duration_secs: 10,
            upload: Some(DirectionResult {
                bytes_transferred: 10_000_000,
                bandwidth_mbps: 80.0,
                peak_mbps: 95.0,
                jitter_ms: 2.5,
                per_second_mbps: vec![],
            }),
            download: None,
        };
        assert_eq!(result.mode, "client");
        assert!(result.upload.is_some());
        assert!(result.download.is_none());
    }

    #[test]
    fn test_bandwidth_result_server() {
        let result = BandwidthResult {
            mode: "server".to_string(),
            target: None,
            bind: Some("0.0.0.0:5201".to_string()),
            duration_secs: 0,
            upload: None,
            download: None,
        };
        assert_eq!(result.mode, "server");
        assert!(result.target.is_none());
        assert!(result.bind.is_some());
    }

    #[test]
    fn test_bandwidth_result_both_directions() {
        let upload = DirectionResult {
            bytes_transferred: 50_000_000,
            bandwidth_mbps: 400.0,
            peak_mbps: 450.0,
            jitter_ms: 1.0,
            per_second_mbps: vec![400.0, 410.0, 390.0],
        };
        let download = DirectionResult {
            bytes_transferred: 60_000_000,
            bandwidth_mbps: 480.0,
            peak_mbps: 520.0,
            jitter_ms: 1.2,
            per_second_mbps: vec![480.0, 490.0, 470.0],
        };
        let result = BandwidthResult {
            mode: "client".to_string(),
            target: Some("192.168.1.100:5201".to_string()),
            bind: None,
            duration_secs: 10,
            upload: Some(upload),
            download: Some(download),
        };
        assert!(result.upload.is_some());
        assert!(result.download.is_some());
        assert!(result.upload.as_ref().unwrap().bandwidth_mbps < result.download.as_ref().unwrap().bandwidth_mbps);
    }
}
