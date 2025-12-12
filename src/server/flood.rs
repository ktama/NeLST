//! フラッドサーバモジュール
//!
//! 指定サイズのデータを継続送信するテストサーバ。

use crate::cli::load::Protocol;
use crate::cli::server::FloodServerArgs;
use crate::common::error::Result;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, UdpSocket};
use tracing::{debug, info};

/// フラッドサーバを起動
pub async fn run(args: &FloodServerArgs) -> Result<()> {
    match args.protocol {
        Protocol::Tcp => run_tcp(args).await,
        Protocol::Udp => run_udp(args).await,
    }
}

/// TCPフラッドサーバを起動
async fn run_tcp(args: &FloodServerArgs) -> Result<()> {
    let listener = TcpListener::bind(args.bind).await?;
    info!("TCP Flood server listening on {}", args.bind);

    let data_size = args.size;
    loop {
        let (mut socket, addr) = listener.accept().await?;
        debug!("New connection from {}", addr);

        tokio::spawn(async move {
            let data = vec![0x41u8; data_size]; // 'A' で埋める
            let mut total_bytes = 0u64;
            loop {
                match socket.write_all(&data).await {
                    Ok(()) => {
                        total_bytes += data_size as u64;
                        if total_bytes % (1024 * 1024 * 10) < data_size as u64 {
                            debug!("Sent {} MB to {}", total_bytes / (1024 * 1024), addr);
                        }
                    }
                    Err(e) => {
                        debug!(
                            "Write error to {}: {} (sent {} bytes total)",
                            addr, e, total_bytes
                        );
                        break;
                    }
                }
            }
        });
    }
}

/// UDPフラッドサーバを起動
async fn run_udp(args: &FloodServerArgs) -> Result<()> {
    let socket = UdpSocket::bind(args.bind).await?;
    info!("UDP Flood server listening on {}", args.bind);
    info!("Send any UDP packet to start receiving flood data");

    let data = vec![0x41u8; args.size.min(65507)]; // UDPの最大ペイロードサイズ
    let mut buf = [0u8; 1];

    loop {
        // クライアントからのパケットを待つ
        let (_, addr) = socket.recv_from(&mut buf).await?;
        debug!("Received trigger from {}, starting flood", addr);

        // フラッドを開始（一定量送信）
        let mut total_bytes = 0u64;
        for _ in 0..10000 {
            match socket.send_to(&data, addr).await {
                Ok(n) => {
                    total_bytes += n as u64;
                }
                Err(e) => {
                    debug!("Send error to {}: {}", addr, e);
                    break;
                }
            }
        }
        debug!("Sent {} bytes to {}", total_bytes, addr);
    }
}
