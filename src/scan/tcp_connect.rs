//! TCP Connectスキャンモジュール
//!
//! 通常のTCP 3ウェイハンドシェイクを完了させてポートの開閉を判定する。

use crate::cli::scan::{parse_ports, PortScanArgs};
use crate::common::error::{NelstError, Result};
use crate::common::output::create_progress_bar;
use crate::common::stats::Timer;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Semaphore};
use tracing::debug;

/// よく使われるポートとサービス名のマッピング
fn get_service_name(port: u16) -> Option<String> {
    let service = match port {
        20 => "ftp-data",
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "dns",
        80 => "http",
        110 => "pop3",
        111 => "rpcbind",
        135 => "msrpc",
        139 => "netbios-ssn",
        143 => "imap",
        443 => "https",
        445 => "microsoft-ds",
        993 => "imaps",
        995 => "pop3s",
        1433 => "ms-sql-s",
        1521 => "oracle",
        3306 => "mysql",
        3389 => "ms-wbt-server",
        5432 => "postgresql",
        5900 => "vnc",
        6379 => "redis",
        8080 => "http-proxy",
        8443 => "https-alt",
        27017 => "mongodb",
        _ => return None,
    };
    Some(service.to_string())
}

/// ポートの状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortState {
    Open,
    Closed,
    Filtered,
}

impl std::fmt::Display for PortState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortState::Open => write!(f, "open"),
            PortState::Closed => write!(f, "closed"),
            PortState::Filtered => write!(f, "filtered"),
        }
    }
}

/// スキャン結果（1ポート分）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortResult {
    pub port: u16,
    pub state: PortState,
    pub service: Option<String>,
}

/// スキャン結果（全体）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub target: String,
    pub method: String,
    pub scan_time: String,
    pub duration_secs: f64,
    pub ports: Vec<PortResult>,
    pub summary: ScanSummary,
}

/// スキャンサマリー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub total_scanned: usize,
    pub open: usize,
    pub closed: usize,
    pub filtered: usize,
}

impl std::fmt::Display for ScanResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  PORT      STATE    SERVICE")?;
        for port in &self.ports {
            if port.state == PortState::Open {
                writeln!(
                    f,
                    "  {}/tcp    {:8} {}",
                    port.port,
                    port.state,
                    port.service.as_deref().unwrap_or("-")
                )?;
            }
        }
        writeln!(f)?;
        writeln!(f, "Scan completed in {:.2}s", self.duration_secs)?;
        writeln!(
            f,
            "Open: {}, Closed: {}, Filtered: {}",
            self.summary.open, self.summary.closed, self.summary.filtered
        )?;
        Ok(())
    }
}

/// TCP Connectスキャンを実行
pub async fn run(args: &PortScanArgs) -> Result<ScanResult> {
    let timer = Timer::new();
    let target = args.target;
    let timeout = Duration::from_millis(args.timeout);
    let concurrency = args.concurrency;

    // ポートをパース
    let ports = parse_ports(&args.ports)
        .map_err(NelstError::argument)?;

    let total_ports = ports.len();

    // プログレスバー
    let pb = create_progress_bar(total_ports as u64, "Scanning");

    // 結果を格納
    let results: Arc<Mutex<Vec<PortResult>>> = Arc::new(Mutex::new(Vec::new()));

    // 同時接続数を制限するセマフォ
    let semaphore = Arc::new(Semaphore::new(concurrency));

    // タスクを生成
    let mut handles = Vec::new();
    for port in ports {
        let semaphore = semaphore.clone();
        let results = results.clone();
        let pb = pb.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let addr = SocketAddr::new(target, port);
            let state = scan_port(addr, timeout).await;
            let service = if state == PortState::Open {
                get_service_name(port)
            } else {
                None
            };

            debug!("Port {} is {:?}", port, state);

            results.lock().await.push(PortResult {
                port,
                state,
                service,
            });

            pb.inc(1);
        });
        handles.push(handle);
    }

    // 全タスクの完了を待機
    for handle in handles {
        let _ = handle.await;
    }

    pb.finish_and_clear();

    // 結果を集計
    let mut port_results = results.lock().await.clone();
    port_results.sort_by_key(|r| r.port);

    let open_count = port_results.iter().filter(|r| r.state == PortState::Open).count();
    let closed_count = port_results.iter().filter(|r| r.state == PortState::Closed).count();
    let filtered_count = port_results.iter().filter(|r| r.state == PortState::Filtered).count();

    Ok(ScanResult {
        target: target.to_string(),
        method: "tcp-connect".to_string(),
        scan_time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        duration_secs: timer.elapsed_secs(),
        ports: port_results,
        summary: ScanSummary {
            total_scanned: total_ports,
            open: open_count,
            closed: closed_count,
            filtered: filtered_count,
        },
    })
}

/// 単一ポートをスキャン
async fn scan_port(addr: SocketAddr, timeout: Duration) -> PortState {
    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => PortState::Open,
        Ok(Err(e)) => {
            // 接続拒否はClosed、その他はFiltered
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                PortState::Closed
            } else {
                PortState::Filtered
            }
        }
        Err(_) => PortState::Filtered, // タイムアウト
    }
}
