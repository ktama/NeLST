//! SYNスキャンモジュール
//!
//! SYNパケットのみを送信し、SYN/ACKまたはRSTの応答でポート状態を判定する。
//! root権限が必要。

use super::raw_socket::{
    TcpResponse, TcpScanFlags, build_tcp_packet, create_tcp_channel, get_local_ip, random_seq,
    random_source_port,
};
use super::tcp_connect::{PortResult, PortState, ScanResult, ScanSummary, get_service_name};
use crate::cli::scan::{PortScanArgs, ScanMethod, parse_ports};
use crate::common::error::{NelstError, Result};
use crate::common::output::create_progress_bar;
use crate::common::stats::Timer;
use chrono::Local;
use pnet::transport::TransportReceiver;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::debug;

/// スキャンフラグの種類を取得
fn get_scan_flags(method: &ScanMethod) -> TcpScanFlags {
    match method {
        ScanMethod::Syn => TcpScanFlags::Syn,
        ScanMethod::Fin => TcpScanFlags::Fin,
        ScanMethod::Xmas => TcpScanFlags::Xmas,
        ScanMethod::Null => TcpScanFlags::Null,
        _ => TcpScanFlags::Syn,
    }
}

/// Raw Socketスキャンを実行（SYN/FIN/Xmas/NULL共通）
pub async fn run(args: &PortScanArgs) -> Result<ScanResult> {
    let timer = Timer::new();
    let scan_flags = get_scan_flags(&args.method);

    // ターゲットIPをIPv4に変換
    let target_ip = match args.target {
        IpAddr::V4(ip) => ip,
        IpAddr::V6(_) => {
            return Err(NelstError::argument(
                "IPv6 is not supported for raw socket scanning".to_string(),
            ));
        }
    };

    // ローカルIPを取得
    let local_ip = get_local_ip(args.target)?;
    debug!("Local IP: {}, Target IP: {}", local_ip, target_ip);

    // Raw Socketチャネルを作成
    let (mut tx, rx) = create_tcp_channel()?;

    // ポートをパース
    let ports = parse_ports(&args.ports).map_err(NelstError::argument)?;
    let total_ports = ports.len();

    // プログレスバー
    let pb = create_progress_bar(
        total_ports as u64,
        &format!("{} Scanning", scan_flags.name()),
    );

    // 結果を格納するマップ（ポート -> 状態）
    let results: Arc<Mutex<HashMap<u16, PortState>>> = Arc::new(Mutex::new(HashMap::new()));

    // 受信スレッドを開始
    let results_clone = results.clone();
    let target_ip_clone = target_ip;
    let rx_handle = tokio::task::spawn_blocking(move || {
        receive_responses(rx, target_ip_clone, results_clone, scan_flags)
    });

    // パケット送信
    let source_port = random_source_port();
    for port in &ports {
        let seq = random_seq();
        let packet = build_tcp_packet(local_ip, source_port, target_ip, *port, scan_flags, seq);

        // パケットを送信
        let _dest = std::net::SocketAddrV4::new(target_ip, *port);
        if let Err(e) = tx.send_to(
            pnet::packet::tcp::TcpPacket::new(&packet).unwrap(),
            IpAddr::V4(target_ip),
        ) {
            debug!("Failed to send packet to port {}: {}", port, e);
        }

        pb.inc(1);

        // 送信間隔を空ける（レート制限）
        tokio::time::sleep(Duration::from_micros(100)).await;
    }

    pb.finish_and_clear();

    // 応答待機時間
    let wait_time = Duration::from_millis(args.timeout * 2);
    debug!("Waiting {}ms for responses...", wait_time.as_millis());
    tokio::time::sleep(wait_time).await;

    // 受信スレッドを停止（タイムアウト）
    drop(rx_handle);

    // 結果を集計
    let results_map = results.lock().await;
    let mut port_results: Vec<PortResult> = Vec::new();
    let mut open = 0;
    let mut closed = 0;
    let mut filtered = 0;

    for port in &ports {
        let state = results_map.get(port).copied().unwrap_or(
            // 応答なしの場合
            match scan_flags {
                TcpScanFlags::Syn => PortState::Filtered,
                // FIN/Xmas/NULL: 応答なし = オープンまたはフィルタリング
                TcpScanFlags::Fin | TcpScanFlags::Xmas | TcpScanFlags::Null => PortState::Open,
            },
        );

        match state {
            PortState::Open => open += 1,
            PortState::Closed => closed += 1,
            PortState::Filtered => filtered += 1,
        }

        port_results.push(PortResult {
            port: *port,
            state,
            service: if state == PortState::Open {
                get_service_name(*port)
            } else {
                None
            },
        });
    }

    let duration = timer.elapsed_secs();

    Ok(ScanResult {
        target: args.target.to_string(),
        method: scan_flags.name().to_string(),
        scan_time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        duration_secs: duration,
        ports: port_results,
        summary: ScanSummary {
            total_scanned: total_ports,
            open,
            closed,
            filtered,
        },
    })
}

/// 応答パケットを受信
fn receive_responses(
    mut rx: TransportReceiver,
    target_ip: Ipv4Addr,
    results: Arc<Mutex<HashMap<u16, PortState>>>,
    scan_flags: TcpScanFlags,
) {
    use pnet::transport::tcp_packet_iter;

    let mut iter = tcp_packet_iter(&mut rx);

    // 5秒間受信を続ける
    let start = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(5);

    while start.elapsed() < timeout_duration {
        match iter.next_with_timeout(Duration::from_millis(100)) {
            Ok(Some((packet, addr))) => {
                // ターゲットからの応答のみ処理
                if let IpAddr::V4(src_ip) = addr {
                    if src_ip != target_ip {
                        continue;
                    }
                }

                let response = TcpResponse::from_packet(&packet);
                let state = determine_port_state(&response, scan_flags);

                debug!(
                    "Received response from port {}: flags={:#06x}, state={:?}",
                    response.source_port, response.flags, state
                );

                // 結果を記録
                let results_clone = results.clone();
                let port = response.source_port;
                // 同期的にロックを取得（spawn_blockingの中なので可能）
                if let Ok(mut map) = results_clone.try_lock() {
                    map.insert(port, state);
                }
            }
            Ok(None) => continue,
            Err(e) => {
                debug!("Error receiving packet: {}", e);
                break;
            }
        }
    }
}

/// 応答からポート状態を判定
fn determine_port_state(response: &TcpResponse, scan_flags: TcpScanFlags) -> PortState {
    match scan_flags {
        TcpScanFlags::Syn => {
            if response.is_syn_ack {
                PortState::Open
            } else if response.is_rst {
                PortState::Closed
            } else {
                PortState::Filtered
            }
        }
        TcpScanFlags::Fin | TcpScanFlags::Xmas | TcpScanFlags::Null => {
            // RST応答 = クローズ、応答なし = オープン/フィルタ
            if response.is_rst {
                PortState::Closed
            } else {
                PortState::Open
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pnet::packet::tcp::TcpFlags;

    #[test]
    fn test_get_scan_flags() {
        assert!(matches!(
            get_scan_flags(&ScanMethod::Syn),
            TcpScanFlags::Syn
        ));
        assert!(matches!(
            get_scan_flags(&ScanMethod::Fin),
            TcpScanFlags::Fin
        ));
        assert!(matches!(
            get_scan_flags(&ScanMethod::Xmas),
            TcpScanFlags::Xmas
        ));
        assert!(matches!(
            get_scan_flags(&ScanMethod::Null),
            TcpScanFlags::Null
        ));
        // TCP Connectの場合はデフォルトでSYN
        assert!(matches!(
            get_scan_flags(&ScanMethod::Tcp),
            TcpScanFlags::Syn
        ));
    }

    #[test]
    fn test_determine_port_state_syn_scan() {
        // SYN-ACK応答 → オープン
        let syn_ack = TcpResponse {
            source_port: 80,
            dest_port: 12345,
            flags: TcpFlags::SYN | TcpFlags::ACK,
            is_syn_ack: true,
            is_rst: false,
        };
        assert_eq!(
            determine_port_state(&syn_ack, TcpScanFlags::Syn),
            PortState::Open
        );

        // RST応答 → クローズ
        let rst = TcpResponse {
            source_port: 80,
            dest_port: 12345,
            flags: TcpFlags::RST,
            is_syn_ack: false,
            is_rst: true,
        };
        assert_eq!(
            determine_port_state(&rst, TcpScanFlags::Syn),
            PortState::Closed
        );

        // その他 → フィルタ
        let other = TcpResponse {
            source_port: 80,
            dest_port: 12345,
            flags: TcpFlags::ACK,
            is_syn_ack: false,
            is_rst: false,
        };
        assert_eq!(
            determine_port_state(&other, TcpScanFlags::Syn),
            PortState::Filtered
        );
    }

    #[test]
    fn test_determine_port_state_fin_xmas_null_scan() {
        // FIN/Xmas/NULLスキャンではRST → クローズ
        let rst = TcpResponse {
            source_port: 80,
            dest_port: 12345,
            flags: TcpFlags::RST,
            is_syn_ack: false,
            is_rst: true,
        };
        assert_eq!(
            determine_port_state(&rst, TcpScanFlags::Fin),
            PortState::Closed
        );
        assert_eq!(
            determine_port_state(&rst, TcpScanFlags::Xmas),
            PortState::Closed
        );
        assert_eq!(
            determine_port_state(&rst, TcpScanFlags::Null),
            PortState::Closed
        );

        // RST以外 → オープン
        let other = TcpResponse {
            source_port: 80,
            dest_port: 12345,
            flags: 0,
            is_syn_ack: false,
            is_rst: false,
        };
        assert_eq!(
            determine_port_state(&other, TcpScanFlags::Fin),
            PortState::Open
        );
        assert_eq!(
            determine_port_state(&other, TcpScanFlags::Xmas),
            PortState::Open
        );
        assert_eq!(
            determine_port_state(&other, TcpScanFlags::Null),
            PortState::Open
        );
    }
}
