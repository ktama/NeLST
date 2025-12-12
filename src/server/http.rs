//! HTTPテストサーバモジュール
//!
//! テスト用のHTTPサーバを提供する。固定レスポンス、遅延シミュレーション、エラー率設定が可能。

use crate::cli::server::HttpServerArgs;
use crate::common::error::Result;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{debug, info};

/// HTTPサーバの設定
#[derive(Clone)]
struct ServerConfig {
    response_body: String,
    response_status: StatusCode,
    delay_ms: u64,
    error_rate: f64,
}

/// HTTPテストサーバを起動
pub async fn run(args: &HttpServerArgs) -> Result<()> {
    let addr: SocketAddr = args.bind;
    let listener = TcpListener::bind(addr).await?;

    let status = StatusCode::from_u16(args.status).unwrap_or(StatusCode::OK);
    let config = Arc::new(ServerConfig {
        response_body: args.body.clone(),
        response_status: status,
        delay_ms: args.delay,
        error_rate: args.error_rate,
    });

    info!("HTTP server listening on http://{}", addr);

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let config = config.clone();

        tokio::spawn(async move {
            debug!("New connection from {}", remote_addr);

            let service = service_fn(move |req| {
                let config = config.clone();
                async move { handle_request(req, &config).await }
            });

            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                debug!("Connection error from {}: {}", remote_addr, e);
            }
        });
    }
}

/// リクエストを処理
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    config: &ServerConfig,
) -> std::result::Result<Response<Full<Bytes>>, Infallible> {
    debug!("{} {}", req.method(), req.uri());

    // 遅延を適用
    if config.delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(config.delay_ms)).await;
    }

    // エラー率に基づいてエラーを返す
    if config.error_rate > 0.0 {
        use rand::Rng as _;
        let mut rng = rand::thread_rng();
        if rng.r#gen::<f64>() < config.error_rate {
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Internal Server Error (simulated)")))
                .unwrap());
        }
    }

    // 正常レスポンス
    Ok(Response::builder()
        .status(config.response_status)
        .header("Content-Type", "text/plain")
        .header("Server", "nelst-http-server")
        .body(Full::new(Bytes::from(config.response_body.clone())))
        .unwrap())
}
