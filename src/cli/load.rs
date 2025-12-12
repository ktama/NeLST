//! 負荷テストサブコマンドの定義

use clap::{Args, Subcommand, ValueEnum};
use std::net::SocketAddr;

/// 負荷テストのサブコマンド
#[derive(Subcommand, Debug)]
pub enum LoadCommands {
    /// データ送受信の負荷テスト
    Traffic(TrafficArgs),

    /// 大量コネクション確立テスト
    Connection(ConnectionArgs),
}

/// トラフィック負荷テストの動作モード
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum TrafficMode {
    /// 送信のみ
    Send,
    /// エコーサーバへ送受信
    #[default]
    Echo,
    /// 受信のみ
    Recv,
}

/// プロトコル
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum Protocol {
    #[default]
    Tcp,
    Udp,
}

/// トラフィック負荷テストの引数
#[derive(Args, Debug)]
pub struct TrafficArgs {
    /// ターゲットアドレス (例: 192.168.1.100:8080)
    #[arg(short, long)]
    pub target: SocketAddr,

    /// プロトコル
    #[arg(short, long, value_enum, default_value_t = Protocol::Tcp)]
    pub protocol: Protocol,

    /// テスト継続時間（秒）
    #[arg(short, long, default_value_t = 60)]
    pub duration: u64,

    /// 同時接続数
    #[arg(short, long, default_value_t = 1)]
    pub concurrency: usize,

    /// パケットサイズ（バイト）
    #[arg(short, long, default_value_t = 1024)]
    pub size: usize,

    /// 動作モード
    #[arg(short, long, value_enum, default_value_t = TrafficMode::Echo)]
    pub mode: TrafficMode,

    /// 毎秒リクエスト数（制限なしの場合は省略）
    #[arg(short, long)]
    pub rate: Option<u64>,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

/// コネクション負荷テストの引数
#[derive(Args, Debug)]
pub struct ConnectionArgs {
    /// ターゲットアドレス (例: 192.168.1.100:8080)
    #[arg(short, long)]
    pub target: SocketAddr,

    /// 確立するコネクション総数
    #[arg(short = 'n', long, default_value_t = 1000)]
    pub count: usize,

    /// 同時接続数
    #[arg(short, long, default_value_t = 100)]
    pub concurrency: usize,

    /// コネクションを維持する
    #[arg(long)]
    pub keep_alive: bool,

    /// コネクションタイムアウト（ミリ秒）
    #[arg(long, default_value_t = 5000)]
    pub timeout: u64,

    /// テスト継続時間（秒）
    #[arg(short, long, default_value_t = 60)]
    pub duration: u64,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}
