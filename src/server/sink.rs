//! シンクサーバモジュール
//!
//! 受信したデータを破棄する（応答なし）サーバ。

use crate::cli::load::Protocol;
use crate::cli::server::SinkServerArgs;
use crate::common::error::Result;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, UdpSocket};
use tracing::{debug, info};

/// シンクサーバを起動
pub async fn run(args: &SinkServerArgs) -> Result<()> {
    match args.protocol {
        Protocol::Tcp => run_tcp(args).await,
        Protocol::Udp => run_udp(args).await,
    }
}

/// TCPシンクサーバを起動
async fn run_tcp(args: &SinkServerArgs) -> Result<()> {
    let listener = TcpListener::bind(args.bind).await?;
    info!("TCP Sink server listening on {}", args.bind);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        debug!("New connection from {}", addr);

        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let mut total_bytes = 0u64;
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        debug!(
                            "Connection closed from {}, received {} bytes total",
                            addr, total_bytes
                        );
                        break;
                    }
                    Ok(n) => {
                        total_bytes += n as u64;
                        debug!(
                            "Received {} bytes from {} (total: {})",
                            n, addr, total_bytes
                        );
                    }
                    Err(e) => {
                        debug!("Read error from {}: {}", addr, e);
                        break;
                    }
                }
            }
        });
    }
}

/// UDPシンクサーバを起動
async fn run_udp(args: &SinkServerArgs) -> Result<()> {
    let socket = UdpSocket::bind(args.bind).await?;
    info!("UDP Sink server listening on {}", args.bind);

    let mut buf = vec![0u8; 65535];
    let mut total_bytes = 0u64;
    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        total_bytes += len as u64;
        debug!(
            "Received {} bytes from {} (total: {})",
            len, addr, total_bytes
        );
    }
}
