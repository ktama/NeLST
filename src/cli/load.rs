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

    /// HTTP負荷テスト
    Http(HttpArgs),
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

/// HTTP負荷テストの引数
#[derive(Args, Debug)]
pub struct HttpArgs {
    /// ターゲットURL (例: http://localhost:8080/api)
    #[arg(short = 'u', long)]
    pub url: String,

    /// HTTPメソッド
    #[arg(short = 'X', long, default_value = "GET")]
    pub method: String,

    /// カスタムヘッダー (例: -H "Content-Type: application/json")
    #[arg(short = 'H', long = "header", value_name = "HEADER")]
    pub headers: Vec<String>,

    /// リクエストボディ（@から始まる場合はファイルパス）
    #[arg(short, long)]
    pub body: Option<String>,

    /// テスト継続時間（秒）
    #[arg(short, long, default_value_t = 60)]
    pub duration: u64,

    /// 同時接続数
    #[arg(short, long, default_value_t = 10)]
    pub concurrency: usize,

    /// 毎秒リクエスト数（制限なしの場合は省略）
    #[arg(short, long)]
    pub rate: Option<u64>,

    /// SSL証明書の検証をスキップ
    #[arg(long)]
    pub insecure: bool,

    /// リダイレクトを追従
    #[arg(long)]
    pub follow_redirects: bool,

    /// リクエストタイムアウト（ミリ秒）
    #[arg(long, default_value_t = 30000)]
    pub timeout: u64,

    /// HTTP/2を優先して使用
    #[arg(long)]
    pub http2: bool,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: LoadCommands,
    }

    #[test]
    fn test_http_args_default_values() {
        let cli = TestCli::parse_from(["test", "http", "-u", "http://localhost:8080"]);
        if let LoadCommands::Http(args) = cli.command {
            assert_eq!(args.url, "http://localhost:8080");
            assert_eq!(args.method, "GET");
            assert!(args.headers.is_empty());
            assert!(args.body.is_none());
            assert_eq!(args.duration, 60);
            assert_eq!(args.concurrency, 10);
            assert!(args.rate.is_none());
            assert!(!args.insecure);
            assert!(!args.follow_redirects);
            assert_eq!(args.timeout, 30000);
            assert!(!args.http2);
            assert!(args.output.is_none());
        } else {
            panic!("Expected Http command");
        }
    }

    #[test]
    fn test_http_args_custom_values() {
        let cli = TestCli::parse_from([
            "test",
            "http",
            "-u",
            "https://example.com/api",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-H",
            "Authorization: Bearer token",
            "-b",
            r#"{"key":"value"}"#,
            "-d",
            "30",
            "-c",
            "50",
            "-r",
            "1000",
            "--insecure",
            "--follow-redirects",
            "--timeout",
            "5000",
            "--http2",
            "-o",
            "result.json",
        ]);
        if let LoadCommands::Http(args) = cli.command {
            assert_eq!(args.url, "https://example.com/api");
            assert_eq!(args.method, "POST");
            assert_eq!(args.headers.len(), 2);
            assert_eq!(args.headers[0], "Content-Type: application/json");
            assert_eq!(args.headers[1], "Authorization: Bearer token");
            assert_eq!(args.body, Some(r#"{"key":"value"}"#.to_string()));
            assert_eq!(args.duration, 30);
            assert_eq!(args.concurrency, 50);
            assert_eq!(args.rate, Some(1000));
            assert!(args.insecure);
            assert!(args.follow_redirects);
            assert_eq!(args.timeout, 5000);
            assert!(args.http2);
            assert_eq!(args.output, Some("result.json".to_string()));
        } else {
            panic!("Expected Http command");
        }
    }

    #[test]
    fn test_http_args_body_from_file_syntax() {
        let cli = TestCli::parse_from([
            "test",
            "http",
            "-u",
            "http://localhost:8080",
            "-b",
            "@data.json",
        ]);
        if let LoadCommands::Http(args) = cli.command {
            assert_eq!(args.body, Some("@data.json".to_string()));
        } else {
            panic!("Expected Http command");
        }
    }

    #[test]
    fn test_traffic_args_default_values() {
        let cli = TestCli::parse_from(["test", "traffic", "-t", "127.0.0.1:8080"]);
        if let LoadCommands::Traffic(args) = cli.command {
            assert_eq!(args.target.to_string(), "127.0.0.1:8080");
            assert!(matches!(args.protocol, Protocol::Tcp));
            assert_eq!(args.duration, 60);
            assert_eq!(args.concurrency, 1);
            assert_eq!(args.size, 1024);
            assert!(matches!(args.mode, TrafficMode::Echo));
        } else {
            panic!("Expected Traffic command");
        }
    }

    #[test]
    fn test_connection_args_default_values() {
        let cli = TestCli::parse_from(["test", "connection", "-t", "127.0.0.1:8080"]);
        if let LoadCommands::Connection(args) = cli.command {
            assert_eq!(args.target.to_string(), "127.0.0.1:8080");
            assert_eq!(args.count, 1000);
            assert_eq!(args.concurrency, 100);
            assert!(!args.keep_alive);
            assert_eq!(args.timeout, 5000);
        } else {
            panic!("Expected Connection command");
        }
    }
}
