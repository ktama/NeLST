//! MTU探索実装
//!
//! Path MTU Discoveryを使用して最適なMTUサイズを検出する。

use crate::cli::diag::MtuArgs;
use crate::common::error::NelstError;
use serde::Serialize;
use socket2::{Domain, Protocol, Socket, Type};
use std::mem::MaybeUninit;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// MTU探索結果
#[derive(Debug, Clone, Serialize)]
pub struct MtuResult {
    /// ターゲット
    pub target: String,
    /// 解決されたIPアドレス
    pub resolved_ip: String,
    /// 検出されたPath MTU
    pub path_mtu: u16,
    /// 探索に使用した範囲（最小）
    pub min_tested: u16,
    /// 探索に使用した範囲（最大）
    pub max_tested: u16,
    /// 探索時間（ミリ秒）
    pub discovery_time_ms: f64,
    /// 各MTUサイズでのテスト結果
    pub probes: Vec<MtuProbe>,
}

/// MTUプローブ結果
#[derive(Debug, Clone, Serialize)]
pub struct MtuProbe {
    /// テストしたMTUサイズ
    pub mtu_size: u16,
    /// 成功したかどうか
    pub success: bool,
    /// RTT（成功時）
    pub rtt_ms: Option<f64>,
}

/// ホスト名をIPアドレスに解決
fn resolve_host(target: &str) -> Result<IpAddr, NelstError> {
    if let Ok(ip) = target.parse::<IpAddr>() {
        return Ok(ip);
    }

    let addr = format!("{}:0", target);
    match addr.to_socket_addrs() {
        Ok(mut addrs) => {
            if let Some(socket_addr) = addrs.next() {
                Ok(socket_addr.ip())
            } else {
                Err(NelstError::connection(format!(
                    "Could not resolve hostname: {}",
                    target
                )))
            }
        }
        Err(e) => Err(NelstError::connection(format!(
            "DNS resolution failed for {}: {}",
            target, e
        ))),
    }
}

/// 指定したサイズでpingを試行
fn probe_mtu(
    target_ip: IpAddr,
    mtu_size: u16,
    timeout: Duration,
) -> Result<Option<f64>, NelstError> {
    let domain = if target_ip.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    // ICMPソケットを作成
    let protocol = if target_ip.is_ipv4() {
        Some(Protocol::ICMPV4)
    } else {
        Some(Protocol::ICMPV6)
    };

    let socket = Socket::new(domain, Type::DGRAM, protocol).map_err(|e| {
        NelstError::permission(format!(
            "Failed to create ICMP socket: {}. Try running with sudo.",
            e
        ))
    })?;

    // Don't Fragment フラグを設定（IPv4のみ、Linux固有）
    #[cfg(target_os = "linux")]
    if target_ip.is_ipv4() {
        // IP_PMTUDISC_DO を直接設定
        use std::os::unix::io::AsRawFd;
        unsafe {
            let val: libc::c_int = 2; // IP_PMTUDISC_DO
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::IPPROTO_IP,
                libc::IP_MTU_DISCOVER,
                &val as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
        }
    }

    socket
        .set_read_timeout(Some(timeout))
        .map_err(|e| NelstError::connection(format!("Failed to set timeout: {}", e)))?;

    socket
        .set_write_timeout(Some(timeout))
        .map_err(|e| NelstError::connection(format!("Failed to set timeout: {}", e)))?;

    let target: SocketAddr = SocketAddr::new(target_ip, 0);
    socket
        .connect(&target.into())
        .map_err(|e| NelstError::connection(format!("Failed to connect socket: {}", e)))?;

    // ICMPペイロード（ヘッダを考慮してサイズを調整）
    // IPv4: 20 (IP) + 8 (ICMP) = 28 bytes overhead
    // IPv6: 40 (IP) + 8 (ICMP) = 48 bytes overhead
    let overhead = if target_ip.is_ipv4() { 28 } else { 48 };
    let payload_size = mtu_size.saturating_sub(overhead) as usize;

    if payload_size == 0 {
        return Ok(None);
    }

    // ICMP Echo Requestパケットを構築
    let mut packet = vec![0u8; payload_size.max(8)];
    // Type: Echo Request (8 for IPv4, 128 for IPv6)
    packet[0] = if target_ip.is_ipv4() { 8 } else { 128 };
    // Code: 0
    packet[1] = 0;
    // Checksum (位置2-3): 後で計算
    // Identifier (位置4-5)
    packet[4] = 0x12;
    packet[5] = 0x34;
    // Sequence (位置6-7)
    packet[6] = 0x00;
    packet[7] = 0x01;

    // チェックサム計算
    let checksum = calculate_icmp_checksum(&packet);
    packet[2] = (checksum >> 8) as u8;
    packet[3] = (checksum & 0xff) as u8;

    let start = Instant::now();

    // 送信
    match socket.send(&packet) {
        Ok(_) => {
            // 応答を受信（MaybeUninitバッファを使用）
            let mut buf: [MaybeUninit<u8>; 65535] = unsafe { MaybeUninit::uninit().assume_init() };
            match socket.recv(&mut buf) {
                Ok(_) => {
                    let rtt = start.elapsed().as_secs_f64() * 1000.0;
                    Ok(Some(rtt))
                }
                Err(e) => {
                    debug!("No response for MTU {}: {}", mtu_size, e);
                    Ok(None)
                }
            }
        }
        Err(e) => {
            // EMSGSIZE = パケットが大きすぎる
            if e.raw_os_error() == Some(libc::EMSGSIZE) {
                debug!("MTU {} too large", mtu_size);
                Ok(None)
            } else {
                debug!("Send failed for MTU {}: {}", mtu_size, e);
                Ok(None)
            }
        }
    }
}

/// ICMPチェックサムを計算
fn calculate_icmp_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;

    while i < data.len() - 1 {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        i += 2;
    }

    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    !sum as u16
}

/// 二分探索でPath MTUを検出
async fn binary_search_mtu(
    target_ip: IpAddr,
    min_mtu: u16,
    max_mtu: u16,
    timeout: Duration,
) -> Result<(u16, Vec<MtuProbe>), NelstError> {
    let mut low = min_mtu;
    let mut high = max_mtu;
    #[allow(unused_assignments)]
    let mut result_mtu = min_mtu;
    let mut probes = Vec::new();

    // まず最大MTUでテスト
    debug!("Testing MTU {}", max_mtu);
    match probe_mtu(target_ip, max_mtu, timeout)? {
        Some(rtt) => {
            probes.push(MtuProbe {
                mtu_size: max_mtu,
                success: true,
                rtt_ms: Some(rtt),
            });
            return Ok((max_mtu, probes));
        }
        None => {
            probes.push(MtuProbe {
                mtu_size: max_mtu,
                success: false,
                rtt_ms: None,
            });
        }
    }

    // 最小MTUでテスト
    debug!("Testing MTU {}", min_mtu);
    match probe_mtu(target_ip, min_mtu, timeout)? {
        Some(rtt) => {
            probes.push(MtuProbe {
                mtu_size: min_mtu,
                success: true,
                rtt_ms: Some(rtt),
            });
            result_mtu = min_mtu;
        }
        None => {
            probes.push(MtuProbe {
                mtu_size: min_mtu,
                success: false,
                rtt_ms: None,
            });
            return Ok((min_mtu, probes));
        }
    }

    // 二分探索
    while low < high - 1 {
        let mid = (low + high) / 2;
        debug!("Testing MTU {} (range: {}-{})", mid, low, high);

        // 非同期でprobeを実行（実際はsocket2はブロッキングなのでspawn_blocking使用）
        let target = target_ip;
        let probe_timeout = timeout;
        let probe_result =
            tokio::task::spawn_blocking(move || probe_mtu(target, mid, probe_timeout))
                .await
                .map_err(|e| NelstError::connection(format!("Task failed: {}", e)))??;

        match probe_result {
            Some(rtt) => {
                probes.push(MtuProbe {
                    mtu_size: mid,
                    success: true,
                    rtt_ms: Some(rtt),
                });
                result_mtu = mid;
                low = mid;
            }
            None => {
                probes.push(MtuProbe {
                    mtu_size: mid,
                    success: false,
                    rtt_ms: None,
                });
                high = mid;
            }
        }
    }

    Ok((result_mtu, probes))
}

/// MTU探索を実行
pub async fn run(args: &MtuArgs) -> Result<MtuResult, NelstError> {
    let target_ip = resolve_host(&args.target)?;
    let timeout = Duration::from_millis(args.timeout);

    info!(
        "MTU discovery to {} ({}) range {}-{}",
        args.target, target_ip, args.min_mtu, args.max_mtu
    );

    let start = Instant::now();
    let (path_mtu, probes) =
        binary_search_mtu(target_ip, args.min_mtu, args.max_mtu, timeout).await?;
    let discovery_time = start.elapsed().as_secs_f64() * 1000.0;

    let result = MtuResult {
        target: args.target.clone(),
        resolved_ip: target_ip.to_string(),
        path_mtu,
        min_tested: args.min_mtu,
        max_tested: args.max_mtu,
        discovery_time_ms: discovery_time,
        probes,
    };

    info!(
        "Path MTU to {} is {} bytes (discovered in {:.2}ms)",
        args.target, path_mtu, discovery_time
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mtu_probe() {
        let probe = MtuProbe {
            mtu_size: 1500,
            success: true,
            rtt_ms: Some(2.5),
        };
        assert!(probe.success);
        assert_eq!(probe.mtu_size, 1500);
    }

    #[test]
    fn test_mtu_probe_failed() {
        let probe = MtuProbe {
            mtu_size: 9000,
            success: false,
            rtt_ms: None,
        };
        assert!(!probe.success);
        assert!(probe.rtt_ms.is_none());
    }

    #[test]
    fn test_mtu_result() {
        let result = MtuResult {
            target: "example.com".to_string(),
            resolved_ip: "93.184.216.34".to_string(),
            path_mtu: 1472,
            min_tested: 68,
            max_tested: 1500,
            discovery_time_ms: 150.5,
            probes: vec![],
        };
        assert_eq!(result.path_mtu, 1472);
    }

    #[test]
    fn test_mtu_result_with_probes() {
        let result = MtuResult {
            target: "192.168.1.1".to_string(),
            resolved_ip: "192.168.1.1".to_string(),
            path_mtu: 1400,
            min_tested: 576,
            max_tested: 1500,
            discovery_time_ms: 250.0,
            probes: vec![
                MtuProbe {
                    mtu_size: 1500,
                    success: false,
                    rtt_ms: None,
                },
                MtuProbe {
                    mtu_size: 576,
                    success: true,
                    rtt_ms: Some(5.0),
                },
                MtuProbe {
                    mtu_size: 1038,
                    success: true,
                    rtt_ms: Some(4.5),
                },
                MtuProbe {
                    mtu_size: 1400,
                    success: true,
                    rtt_ms: Some(4.8),
                },
            ],
        };
        assert_eq!(result.probes.len(), 4);
        assert!(!result.probes[0].success);
        assert!(result.probes[1].success);
    }

    #[test]
    fn test_icmp_checksum() {
        // Simple test packet
        let packet = [0x08, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01];
        let checksum = calculate_icmp_checksum(&packet);
        // Checksum should be non-zero for a valid calculation
        assert!(checksum > 0);
    }

    #[test]
    fn test_icmp_checksum_zero_packet() {
        let packet = [0x00; 8];
        let checksum = calculate_icmp_checksum(&packet);
        assert_eq!(checksum, 0xFFFF); // All zeros should give 0xFFFF
    }

    #[test]
    fn test_icmp_checksum_odd_length() {
        // Odd length packet
        let packet = [0x08, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00];
        let checksum = calculate_icmp_checksum(&packet);
        assert!(checksum > 0);
    }

    #[test]
    fn test_resolve_host_ip() {
        let ip = resolve_host("127.0.0.1").unwrap();
        assert_eq!(ip.to_string(), "127.0.0.1");
    }

    #[test]
    fn test_resolve_host_ipv6() {
        let ip = resolve_host("::1").unwrap();
        assert_eq!(ip.to_string(), "::1");
    }

    #[test]
    fn test_resolve_host_invalid() {
        let result = resolve_host("invalid.host.that.does.not.exist.local");
        assert!(result.is_err());
    }
}
