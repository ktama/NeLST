//! Ping実装
//!
//! ICMP/TCP pingテストを行う。

use crate::cli::diag::PingArgs;
use crate::common::error::NelstError;
use serde::Serialize;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};
use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Ping結果
#[derive(Debug, Clone, Serialize)]
pub struct PingResult {
    /// ターゲット
    pub target: String,
    /// 解決されたIPアドレス
    pub resolved_ip: String,
    /// モード（ICMP/TCP）
    pub mode: String,
    /// 送信数
    pub transmitted: u32,
    /// 受信数
    pub received: u32,
    /// パケットロス率（%）
    pub packet_loss: f64,
    /// 最小RTT（ミリ秒）
    pub min_rtt: f64,
    /// 最大RTT（ミリ秒）
    pub max_rtt: f64,
    /// 平均RTT（ミリ秒）
    pub avg_rtt: f64,
    /// 標準偏差（ミリ秒）
    pub stddev_rtt: f64,
    /// 各pingのRTT（ミリ秒）
    pub rtts: Vec<f64>,
}

impl PingResult {
    /// 新しいPingResultを作成
    pub fn new(target: &str, resolved_ip: &str, mode: &str) -> Self {
        Self {
            target: target.to_string(),
            resolved_ip: resolved_ip.to_string(),
            mode: mode.to_string(),
            transmitted: 0,
            received: 0,
            packet_loss: 0.0,
            min_rtt: f64::MAX,
            max_rtt: 0.0,
            avg_rtt: 0.0,
            stddev_rtt: 0.0,
            rtts: Vec::new(),
        }
    }

    /// RTTを追加
    pub fn add_rtt(&mut self, rtt_ms: f64) {
        self.rtts.push(rtt_ms);
        self.received += 1;
        if rtt_ms < self.min_rtt {
            self.min_rtt = rtt_ms;
        }
        if rtt_ms > self.max_rtt {
            self.max_rtt = rtt_ms;
        }
    }

    /// 統計を計算
    pub fn calculate_stats(&mut self) {
        if self.rtts.is_empty() {
            self.min_rtt = 0.0;
            self.max_rtt = 0.0;
            self.avg_rtt = 0.0;
            self.stddev_rtt = 0.0;
            self.packet_loss = 100.0;
            return;
        }

        let sum: f64 = self.rtts.iter().sum();
        self.avg_rtt = sum / self.rtts.len() as f64;

        let variance: f64 = self.rtts.iter().map(|rtt| (rtt - self.avg_rtt).powi(2)).sum::<f64>()
            / self.rtts.len() as f64;
        self.stddev_rtt = variance.sqrt();

        self.packet_loss = if self.transmitted > 0 {
            ((self.transmitted - self.received) as f64 / self.transmitted as f64) * 100.0
        } else {
            0.0
        };
    }
}

/// ホスト名をIPアドレスに解決
fn resolve_host(target: &str) -> Result<IpAddr, NelstError> {
    // まずIPアドレスとして解析を試みる
    if let Ok(ip) = target.parse::<IpAddr>() {
        return Ok(ip);
    }

    // ホスト名として解決
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

/// ICMP pingを実行
async fn icmp_ping(
    target_ip: IpAddr,
    count: u32,
    interval: Duration,
    timeout_duration: Duration,
    size: usize,
) -> Result<Vec<Option<f64>>, NelstError> {
    let config = Config::default();
    let client = Client::new(&config).map_err(|e| {
        NelstError::permission(format!(
            "Failed to create ICMP client: {}. Try running with sudo.",
            e
        ))
    })?;

    let payload = vec![0u8; size.saturating_sub(8)]; // ICMPヘッダ分を引く
    let mut results = Vec::with_capacity(count as usize);
    let mut pinger = client.pinger(target_ip, PingIdentifier(rand::random())).await;

    for seq in 0..count {
        let _start = Instant::now();
        
        match timeout(timeout_duration, pinger.ping(PingSequence(seq as u16), &payload)).await {
            Ok(Ok((IcmpPacket::V4(_), rtt))) | Ok(Ok((IcmpPacket::V6(_), rtt))) => {
                let rtt_ms = rtt.as_secs_f64() * 1000.0;
                debug!("Reply from {}: seq={} time={:.2}ms", target_ip, seq, rtt_ms);
                results.push(Some(rtt_ms));
            }
            Ok(Err(e)) => {
                warn!("Ping failed for seq={}: {}", seq, e);
                results.push(None);
            }
            Err(_) => {
                warn!("Ping timeout for seq={}", seq);
                results.push(None);
            }
        }

        if seq < count - 1 {
            tokio::time::sleep(interval).await;
        }
    }

    Ok(results)
}

/// TCP pingを実行
async fn tcp_ping(
    target_ip: IpAddr,
    port: u16,
    count: u32,
    interval: Duration,
    timeout_duration: Duration,
) -> Result<Vec<Option<f64>>, NelstError> {
    let target = SocketAddr::new(target_ip, port);
    let mut results = Vec::with_capacity(count as usize);

    for seq in 0..count {
        let start = Instant::now();

        match timeout(timeout_duration, TcpStream::connect(&target)).await {
            Ok(Ok(mut stream)) => {
                let rtt = start.elapsed();
                let rtt_ms = rtt.as_secs_f64() * 1000.0;
                debug!(
                    "TCP connection to {}:{} seq={} time={:.2}ms",
                    target_ip, port, seq, rtt_ms
                );
                results.push(Some(rtt_ms));
                // 接続を閉じる
                let _ = stream.shutdown().await;
            }
            Ok(Err(e)) => {
                warn!("TCP connection failed for seq={}: {}", seq, e);
                results.push(None);
            }
            Err(_) => {
                warn!("TCP connection timeout for seq={}", seq);
                results.push(None);
            }
        }

        if seq < count - 1 {
            tokio::time::sleep(interval).await;
        }
    }

    Ok(results)
}

/// Pingを実行
pub async fn run(args: &PingArgs) -> Result<PingResult, NelstError> {
    let target_ip = resolve_host(&args.target)?;
    let mode = if args.tcp { "TCP" } else { "ICMP" };
    
    info!(
        "PING {} ({}) {} mode",
        args.target, target_ip, mode
    );

    let interval = Duration::from_millis(args.interval);
    let timeout_duration = Duration::from_millis(args.timeout);

    let ping_results = if args.tcp {
        tcp_ping(target_ip, args.port, args.count, interval, timeout_duration).await?
    } else {
        icmp_ping(target_ip, args.count, interval, timeout_duration, args.size).await?
    };

    let mut result = PingResult::new(&args.target, &target_ip.to_string(), mode);
    result.transmitted = args.count;

    for rtt_ms in ping_results.into_iter().flatten() {
        result.add_rtt(rtt_ms);
    }

    result.calculate_stats();

    info!(
        "--- {} ping statistics ---",
        args.target
    );
    info!(
        "{} packets transmitted, {} received, {:.1}% packet loss",
        result.transmitted, result.received, result.packet_loss
    );
    if result.received > 0 {
        info!(
            "rtt min/avg/max/stddev = {:.3}/{:.3}/{:.3}/{:.3} ms",
            result.min_rtt, result.avg_rtt, result.max_rtt, result.stddev_rtt
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_result_new() {
        let result = PingResult::new("example.com", "93.184.216.34", "ICMP");
        assert_eq!(result.target, "example.com");
        assert_eq!(result.resolved_ip, "93.184.216.34");
        assert_eq!(result.mode, "ICMP");
        assert_eq!(result.transmitted, 0);
        assert_eq!(result.received, 0);
    }

    #[test]
    fn test_ping_result_add_rtt() {
        let mut result = PingResult::new("test", "127.0.0.1", "ICMP");
        result.add_rtt(10.5);
        result.add_rtt(15.3);
        result.add_rtt(12.1);

        assert_eq!(result.rtts.len(), 3);
        assert_eq!(result.received, 3);
        assert_eq!(result.min_rtt, 10.5);
        assert_eq!(result.max_rtt, 15.3);
    }

    #[test]
    fn test_ping_result_calculate_stats() {
        let mut result = PingResult::new("test", "127.0.0.1", "ICMP");
        result.transmitted = 5;
        result.add_rtt(10.0);
        result.add_rtt(20.0);
        result.add_rtt(15.0);
        result.calculate_stats();

        assert_eq!(result.received, 3);
        assert!((result.avg_rtt - 15.0).abs() < 0.001);
        assert!((result.packet_loss - 40.0).abs() < 0.001);
    }

    #[test]
    fn test_ping_result_stddev() {
        let mut result = PingResult::new("test", "127.0.0.1", "ICMP");
        result.transmitted = 4;
        // Values: 10, 20, 30, 40 -> avg=25, variance=125, stddev≈11.18
        result.add_rtt(10.0);
        result.add_rtt(20.0);
        result.add_rtt(30.0);
        result.add_rtt(40.0);
        result.calculate_stats();

        assert!((result.avg_rtt - 25.0).abs() < 0.001);
        assert!((result.stddev_rtt - 11.18).abs() < 0.1);
    }

    #[test]
    fn test_ping_result_no_responses() {
        let mut result = PingResult::new("test", "127.0.0.1", "ICMP");
        result.transmitted = 4;
        result.calculate_stats();

        assert_eq!(result.received, 0);
        assert_eq!(result.packet_loss, 100.0);
        assert_eq!(result.min_rtt, 0.0);
        assert_eq!(result.max_rtt, 0.0);
    }

    #[test]
    fn test_ping_result_single_response() {
        let mut result = PingResult::new("test", "127.0.0.1", "ICMP");
        result.transmitted = 1;
        result.add_rtt(5.5);
        result.calculate_stats();

        assert_eq!(result.received, 1);
        assert_eq!(result.packet_loss, 0.0);
        assert_eq!(result.min_rtt, 5.5);
        assert_eq!(result.max_rtt, 5.5);
        assert_eq!(result.avg_rtt, 5.5);
        assert_eq!(result.stddev_rtt, 0.0);
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
