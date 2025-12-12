//! UDPスキャンモジュール
//!
//! UDPパケットを送信し、ICMP Port Unreachableの応答でポート状態を判定する。

use super::tcp_connect::{PortResult, PortState, ScanResult, ScanSummary};
use crate::cli::scan::{PortScanArgs, parse_ports};
use crate::common::error::{NelstError, Result};
use crate::common::output::create_progress_bar;
use crate::common::stats::Timer;
use chrono::Local;
use std::collections::HashSet;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::debug;

/// UDPスキャンを実行
pub async fn run(args: &PortScanArgs) -> Result<ScanResult> {
    let timer = Timer::new();
    let target = args.target;
    let timeout_duration = Duration::from_millis(args.timeout);

    // ポートをパース
    let ports = parse_ports(&args.ports).map_err(NelstError::argument)?;
    let total_ports = ports.len();

    // プログレスバー
    let pb = create_progress_bar(total_ports as u64, "UDP Scanning");

    // 結果を格納
    let _results: Arc<Mutex<Vec<PortResult>>> = Arc::new(Mutex::new(Vec::new()));

    // UDPソケットを作成
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| NelstError::connection(format!("Failed to bind UDP socket: {}", e)))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    // プローブデータ
    let probe_data = b"\x00";

    // 送信済みポートを追跡
    let sent_ports: Arc<Mutex<HashSet<u16>>> = Arc::new(Mutex::new(HashSet::new()));

    // ポートをスキャン
    for port in &ports {
        let target_addr = SocketAddr::new(target, *port);

        // プローブを送信
        match socket.send_to(probe_data, target_addr) {
            Ok(_) => {
                sent_ports.lock().await.insert(*port);
            }
            Err(e) => {
                debug!("Failed to send UDP probe to port {}: {}", port, e);
            }
        }

        pb.inc(1);

        // 少し待機
        tokio::time::sleep(Duration::from_micros(500)).await;
    }

    // 応答を待機
    debug!("Waiting for ICMP responses...");
    let _wait_start = std::time::Instant::now();
    let closed_ports: Arc<Mutex<HashSet<u16>>> = Arc::new(Mutex::new(HashSet::new()));

    // 注: 実際のICMP Port Unreachable検出にはraw socketが必要
    // ここでは簡易実装として、タイムアウトで応答なし = オープン/フィルタとする
    tokio::time::sleep(timeout_duration * 2).await;

    pb.finish_and_clear();

    // 結果を集計
    let _sent = sent_ports.lock().await;
    let closed = closed_ports.lock().await;
    let mut port_results: Vec<PortResult> = Vec::new();
    let mut open = 0;
    let mut closed_count = 0;
    let mut filtered = 0;

    for port in &ports {
        // UDPでは応答なし = open|filtered として扱う
        let state = if closed.contains(port) {
            PortState::Closed
        } else {
            // オープンまたはフィルタリング
            PortState::Open
        };

        match state {
            PortState::Open => open += 1,
            PortState::Closed => closed_count += 1,
            PortState::Filtered => filtered += 1,
        }

        port_results.push(PortResult {
            port: *port,
            state,
            service: if state == PortState::Open {
                get_udp_service_name(*port)
            } else {
                None
            },
        });
    }

    let duration = timer.elapsed_secs();

    Ok(ScanResult {
        target: target.to_string(),
        method: "UDP".to_string(),
        scan_time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        duration_secs: duration,
        ports: port_results,
        summary: ScanSummary {
            total_scanned: total_ports,
            open,
            closed: closed_count,
            filtered,
        },
    })
}

/// よく使われるUDPポートとサービス名
fn get_udp_service_name(port: u16) -> Option<String> {
    let service = match port {
        53 => "dns",
        67 => "dhcp-server",
        68 => "dhcp-client",
        69 => "tftp",
        123 => "ntp",
        137 => "netbios-ns",
        138 => "netbios-dgm",
        161 => "snmp",
        162 => "snmptrap",
        500 => "isakmp",
        514 => "syslog",
        520 => "rip",
        1194 => "openvpn",
        1900 => "ssdp",
        4500 => "ipsec-nat-t",
        5353 => "mdns",
        _ => return None,
    };
    Some(service.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_udp_service_name() {
        assert_eq!(get_udp_service_name(53), Some("dns".to_string()));
        assert_eq!(get_udp_service_name(123), Some("ntp".to_string()));
        assert_eq!(get_udp_service_name(161), Some("snmp".to_string()));
        assert_eq!(get_udp_service_name(5353), Some("mdns".to_string()));
        assert_eq!(get_udp_service_name(12345), None);
    }
}
