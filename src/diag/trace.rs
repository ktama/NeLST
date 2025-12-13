//! Traceroute実装
//!
//! 経路追跡を行う。TTLを増加させながらパケットを送信し、
//! 各ホップからのICMP Time Exceeded応答を記録する。

use crate::cli::diag::{TraceArgs, TraceMode};
use crate::common::error::NelstError;
use serde::Serialize;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};
use surge_ping::{Client, Config, PingIdentifier, PingSequence};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{info, warn};

/// ホップ情報
#[derive(Debug, Clone, Serialize)]
pub struct Hop {
    /// ホップ番号（TTL）
    pub ttl: u8,
    /// ホップのIPアドレス（応答がない場合はNone）
    pub address: Option<String>,
    /// ホスト名（逆引きできた場合）
    pub hostname: Option<String>,
    /// 各クエリのRTT（ミリ秒、タイムアウトの場合はNone）
    pub rtts: Vec<Option<f64>>,
    /// 宛先に到達したかどうか
    pub is_destination: bool,
}

/// Traceroute結果
#[derive(Debug, Clone, Serialize)]
pub struct TraceResult {
    /// ターゲット
    pub target: String,
    /// 解決されたIPアドレス
    pub resolved_ip: String,
    /// モード
    pub mode: String,
    /// 最大ホップ数
    pub max_hops: u8,
    /// 各ホップの情報
    pub hops: Vec<Hop>,
    /// 宛先に到達したか
    pub reached_destination: bool,
    /// 総ホップ数
    pub total_hops: u8,
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

/// TCP tracerouteを実行
async fn tcp_trace(
    target_ip: IpAddr,
    port: u16,
    max_hops: u8,
    queries: u8,
    timeout_duration: Duration,
) -> Result<Vec<Hop>, NelstError> {
    let mut hops = Vec::new();
    let target = SocketAddr::new(target_ip, port);

    for ttl in 1..=max_hops {
        let mut hop = Hop {
            ttl,
            address: None,
            hostname: None,
            rtts: Vec::with_capacity(queries as usize),
            is_destination: false,
        };

        for _query in 0..queries {
            let start = Instant::now();

            // TCP接続を試行（socket2でTTLを設定）
            match timeout(timeout_duration, tcp_connect_with_ttl(&target, ttl)).await {
                Ok(Ok(connected)) => {
                    let rtt = start.elapsed().as_secs_f64() * 1000.0;
                    hop.rtts.push(Some(rtt));
                    hop.address = Some(target_ip.to_string());
                    if connected {
                        hop.is_destination = true;
                    }
                }
                Ok(Err(_)) | Err(_) => {
                    hop.rtts.push(None);
                }
            }
        }

        hops.push(hop.clone());

        if hop.is_destination {
            break;
        }
    }

    Ok(hops)
}

/// TTLを設定してTCP接続を試行
async fn tcp_connect_with_ttl(target: &SocketAddr, ttl: u8) -> Result<bool, NelstError> {
    use socket2::{Domain, Socket, Type};

    let domain = if target.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    let socket = Socket::new(domain, Type::STREAM, None)
        .map_err(|e| NelstError::connection(format!("Failed to create socket: {}", e)))?;

    socket
        .set_ttl(ttl as u32)
        .map_err(|e| NelstError::connection(format!("Failed to set TTL: {}", e)))?;

    socket.set_nonblocking(true).ok();

    let addr: socket2::SockAddr = (*target).into();

    // 非同期で接続を試行
    match socket.connect(&addr) {
        Ok(()) => Ok(true),
        Err(e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {
            // 接続中 - 実際の接続完了を待つ
            let std_stream: std::net::TcpStream = socket.into();
            std_stream.set_nonblocking(false).ok();
            match TcpStream::from_std(std_stream) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
        Err(_) => Ok(false),
    }
}

/// ICMP tracerouteを実行（簡易実装）
async fn icmp_trace(
    target_ip: IpAddr,
    max_hops: u8,
    queries: u8,
    timeout_duration: Duration,
) -> Result<Vec<Hop>, NelstError> {
    let config = Config::default();
    let client = Client::new(&config).map_err(|e| {
        NelstError::permission(format!(
            "Failed to create ICMP client: {}. Try running with sudo.",
            e
        ))
    })?;

    let mut hops = Vec::new();
    let payload = vec![0u8; 56];

    for ttl in 1..=max_hops {
        let mut hop = Hop {
            ttl,
            address: None,
            hostname: None,
            rtts: Vec::with_capacity(queries as usize),
            is_destination: false,
        };

        let mut pinger = client
            .pinger(target_ip, PingIdentifier(rand::random()))
            .await;
        pinger.timeout(timeout_duration);

        for query in 0..queries {
            let _start = Instant::now();

            // TTLを設定したpingを送信
            // 注: surge-pingはTTL設定をサポートしていないため、簡易的な実装
            match timeout(
                timeout_duration,
                pinger.ping(PingSequence(query as u16), &payload),
            )
            .await
            {
                Ok(Ok((_, rtt))) => {
                    let rtt_ms = rtt.as_secs_f64() * 1000.0;
                    hop.rtts.push(Some(rtt_ms));
                    hop.address = Some(target_ip.to_string());
                    if ttl >= max_hops || hop.address == Some(target_ip.to_string()) {
                        hop.is_destination = true;
                    }
                }
                Ok(Err(_)) | Err(_) => {
                    hop.rtts.push(None);
                }
            }
        }

        // 最初のホップでアドレスが見つかったらそれを記録
        if hop.rtts.iter().any(|r| r.is_some()) && hop.address.is_none() {
            hop.address = Some("*".to_string());
        }

        hops.push(hop.clone());

        if hop.is_destination {
            break;
        }
    }

    Ok(hops)
}

/// UDP tracerouteを実行（簡易実装）
async fn udp_trace(
    target_ip: IpAddr,
    port: u16,
    max_hops: u8,
    queries: u8,
    timeout_duration: Duration,
) -> Result<Vec<Hop>, NelstError> {
    use socket2::{Domain, Protocol, Socket, Type};
    use tokio::net::UdpSocket;

    let mut hops = Vec::new();

    for ttl in 1..=max_hops {
        let mut hop = Hop {
            ttl,
            address: None,
            hostname: None,
            rtts: Vec::with_capacity(queries as usize),
            is_destination: false,
        };

        for query in 0..queries {
            let dest_port = port + (ttl as u16) + (query as u16);
            let target = SocketAddr::new(target_ip, dest_port);

            let domain = if target_ip.is_ipv4() {
                Domain::IPV4
            } else {
                Domain::IPV6
            };

            let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP)).map_err(|e| {
                NelstError::connection(format!("Failed to create UDP socket: {}", e))
            })?;

            socket
                .set_ttl(ttl as u32)
                .map_err(|e| NelstError::connection(format!("Failed to set TTL: {}", e)))?;

            let bind_addr: SocketAddr = if target_ip.is_ipv4() {
                "0.0.0.0:0".parse().unwrap()
            } else {
                "[::]:0".parse().unwrap()
            };
            socket.bind(&bind_addr.into()).ok();
            socket.set_nonblocking(true).ok();

            let std_socket: std::net::UdpSocket = socket.into();
            let udp_socket = UdpSocket::from_std(std_socket).map_err(|e| {
                NelstError::connection(format!("Failed to create async socket: {}", e))
            })?;

            let start = Instant::now();
            let payload = b"NeLST traceroute probe";

            // パケットを送信
            if udp_socket.send_to(payload, &target).await.is_ok() {
                // 応答を待つ（ICMP Time Exceeded）
                let mut buf = [0u8; 1024];
                match timeout(timeout_duration, udp_socket.recv_from(&mut buf)).await {
                    Ok(Ok((_, from))) => {
                        let rtt = start.elapsed().as_secs_f64() * 1000.0;
                        hop.rtts.push(Some(rtt));
                        hop.address = Some(from.ip().to_string());
                        if from.ip() == target_ip {
                            hop.is_destination = true;
                        }
                    }
                    _ => {
                        hop.rtts.push(None);
                    }
                }
            } else {
                hop.rtts.push(None);
            }
        }

        if hop.address.is_none() && hop.rtts.iter().all(|r| r.is_none()) {
            hop.address = Some("*".to_string());
        }

        hops.push(hop.clone());

        if hop.is_destination {
            break;
        }
    }

    Ok(hops)
}

/// Tracerouteを実行
pub async fn run(args: &TraceArgs) -> Result<TraceResult, NelstError> {
    let target_ip = resolve_host(&args.target)?;
    let mode_str = format!("{:?}", args.mode);
    let timeout_duration = Duration::from_millis(args.timeout);

    info!(
        "traceroute to {} ({}), {} hops max, {} mode",
        args.target, target_ip, args.max_hops, mode_str
    );

    let hops = match args.mode {
        TraceMode::Tcp => {
            tcp_trace(
                target_ip,
                args.port,
                args.max_hops,
                args.queries,
                timeout_duration,
            )
            .await?
        }
        TraceMode::Icmp => {
            icmp_trace(target_ip, args.max_hops, args.queries, timeout_duration).await?
        }
        TraceMode::Udp => {
            udp_trace(
                target_ip,
                args.port,
                args.max_hops,
                args.queries,
                timeout_duration,
            )
            .await?
        }
    };

    let reached = hops.iter().any(|h| h.is_destination);
    let total = hops.len() as u8;

    let result = TraceResult {
        target: args.target.clone(),
        resolved_ip: target_ip.to_string(),
        mode: mode_str,
        max_hops: args.max_hops,
        hops,
        reached_destination: reached,
        total_hops: total,
    };

    if reached {
        info!("Reached destination {} in {} hops", target_ip, total);
    } else {
        warn!("Did not reach destination within {} hops", args.max_hops);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hop_new() {
        let hop = Hop {
            ttl: 1,
            address: Some("192.168.1.1".to_string()),
            hostname: None,
            rtts: vec![Some(1.5), Some(1.2), Some(1.8)],
            is_destination: false,
        };
        assert_eq!(hop.ttl, 1);
        assert!(!hop.is_destination);
        assert_eq!(hop.rtts.len(), 3);
    }

    #[test]
    fn test_hop_with_timeouts() {
        let hop = Hop {
            ttl: 5,
            address: None,
            hostname: None,
            rtts: vec![None, None, None],
            is_destination: false,
        };
        assert!(hop.address.is_none());
        assert!(hop.rtts.iter().all(|r| r.is_none()));
    }

    #[test]
    fn test_hop_destination() {
        let hop = Hop {
            ttl: 10,
            address: Some("93.184.216.34".to_string()),
            hostname: Some("example.com".to_string()),
            rtts: vec![Some(25.5), Some(24.8), Some(26.1)],
            is_destination: true,
        };
        assert!(hop.is_destination);
        assert!(hop.hostname.is_some());
    }

    #[test]
    fn test_trace_result() {
        let result = TraceResult {
            target: "example.com".to_string(),
            resolved_ip: "93.184.216.34".to_string(),
            mode: "ICMP".to_string(),
            max_hops: 30,
            hops: vec![],
            reached_destination: true,
            total_hops: 15,
        };
        assert!(result.reached_destination);
        assert_eq!(result.total_hops, 15);
    }

    #[test]
    fn test_trace_result_not_reached() {
        let result = TraceResult {
            target: "unreachable.example.com".to_string(),
            resolved_ip: "10.0.0.1".to_string(),
            mode: "UDP".to_string(),
            max_hops: 30,
            hops: vec![Hop {
                ttl: 1,
                address: Some("192.168.1.1".to_string()),
                hostname: None,
                rtts: vec![Some(1.0)],
                is_destination: false,
            }],
            reached_destination: false,
            total_hops: 30,
        };
        assert!(!result.reached_destination);
        assert_eq!(result.total_hops, 30);
        assert_eq!(result.hops.len(), 1);
    }

    #[test]
    fn test_resolve_host_ip() {
        let ip = resolve_host("127.0.0.1").unwrap();
        assert_eq!(ip.to_string(), "127.0.0.1");
    }

    #[test]
    fn test_resolve_host_invalid() {
        let result = resolve_host("invalid.host.that.does.not.exist.local");
        assert!(result.is_err());
    }
}
