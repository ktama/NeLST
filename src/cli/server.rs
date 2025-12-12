//! テストサーバサブコマンドの定義

use crate::cli::load::Protocol;
use clap::{Args, Subcommand};
use std::net::SocketAddr;

/// テストサーバのサブコマンド
#[derive(Subcommand, Debug)]
pub enum ServerCommands {
    /// エコーサーバ（受信データをそのまま返す）
    Echo(EchoServerArgs),

    /// シンクサーバ（受信のみ、応答なし）
    Sink(SinkServerArgs),

    /// フラッドサーバ（指定サイズのデータを送り続ける）
    Flood(FloodServerArgs),

    /// HTTPテストサーバ
    Http(HttpServerArgs),
}

/// エコーサーバの引数
#[derive(Args, Debug)]
pub struct EchoServerArgs {
    /// バインドアドレス (例: 0.0.0.0:8080)
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    pub bind: SocketAddr,

    /// プロトコル
    #[arg(short, long, value_enum, default_value_t = Protocol::Tcp)]
    pub protocol: Protocol,
}

/// シンクサーバの引数
#[derive(Args, Debug)]
pub struct SinkServerArgs {
    /// バインドアドレス (例: 0.0.0.0:8080)
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    pub bind: SocketAddr,

    /// プロトコル
    #[arg(short, long, value_enum, default_value_t = Protocol::Tcp)]
    pub protocol: Protocol,
}

/// フラッドサーバの引数
#[derive(Args, Debug)]
pub struct FloodServerArgs {
    /// バインドアドレス (例: 0.0.0.0:8080)
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    pub bind: SocketAddr,

    /// プロトコル
    #[arg(short, long, value_enum, default_value_t = Protocol::Tcp)]
    pub protocol: Protocol,

    /// 送信データサイズ（バイト）
    #[arg(short, long, default_value_t = 1024)]
    pub size: usize,
}

/// HTTPサーバの引数
#[derive(Args, Debug)]
pub struct HttpServerArgs {
    /// バインドアドレス (例: 0.0.0.0:8080)
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    pub bind: SocketAddr,

    /// レスポンスボディ
    #[arg(long, default_value = "OK")]
    pub body: String,

    /// レスポンスステータスコード
    #[arg(long, default_value_t = 200)]
    pub status: u16,

    /// レスポンス遅延（ミリ秒）
    #[arg(long, default_value_t = 0)]
    pub delay: u64,

    /// エラー率（0.0-1.0）
    #[arg(long, default_value_t = 0.0)]
    pub error_rate: f64,
}
