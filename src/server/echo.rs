//! エコーサーバモジュール
//!
//! 受信したデータをそのまま返すサーバ。

use crate::cli::load::Protocol;
use crate::cli::server::EchoServerArgs;
use crate::common::error::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use tracing::{debug, info};

/// エコーサーバを起動
pub async fn run(args: &EchoServerArgs) -> Result<()> {
    match args.protocol {
        Protocol::Tcp => run_tcp(args).await,
        Protocol::Udp => run_udp(args).await,
    }
}

/// TCPエコーサーバを起動
async fn run_tcp(args: &EchoServerArgs) -> Result<()> {
    let listener = TcpListener::bind(args.bind).await?;
    info!("TCP Echo server listening on {}", args.bind);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        debug!("New connection from {}", addr);

        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        debug!("Connection closed from {}", addr);
                        break;
                    }
                    Ok(n) => {
                        debug!("Received {} bytes from {}", n, addr);
                        if let Err(e) = socket.write_all(&buf[..n]).await {
                            debug!("Write error to {}: {}", addr, e);
                            break;
                        }
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

/// UDPエコーサーバを起動
async fn run_udp(args: &EchoServerArgs) -> Result<()> {
    let socket = UdpSocket::bind(args.bind).await?;
    info!("UDP Echo server listening on {}", args.bind);

    let mut buf = vec![0u8; 65535];
    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        debug!("Received {} bytes from {}", len, addr);
        if let Err(e) = socket.send_to(&buf[..len], addr).await {
            debug!("Send error to {}: {}", addr, e);
        }
    }
}
