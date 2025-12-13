//! ベンチマークサブコマンドの定義

use clap::{Args, Subcommand, ValueEnum};
use std::net::SocketAddr;

/// ベンチマークのサブコマンド
#[derive(Subcommand, Debug)]
pub enum BenchCommands {
    /// 帯域幅測定
    Bandwidth(BandwidthArgs),
    /// レイテンシ測定
    Latency(LatencyArgs),
}

/// 帯域幅測定方向
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum BandwidthDirection {
    /// アップロードのみ
    Up,
    /// ダウンロードのみ
    Down,
    /// 両方向
    #[default]
    Both,
}

/// 帯域幅測定の引数
#[derive(Args, Debug)]
pub struct BandwidthArgs {
    /// ターゲットサーバ（クライアントモード時）
    #[arg(short, long)]
    pub target: Option<SocketAddr>,

    /// サーバモードで起動
    #[arg(long)]
    pub server: bool,

    /// バインドアドレス（サーバモード時）
    #[arg(short, long, default_value = "0.0.0.0:5201")]
    pub bind: SocketAddr,

    /// 測定時間（秒）
    #[arg(short, long, default_value_t = 10)]
    pub duration: u64,

    /// 測定方向
    #[arg(long, value_enum, default_value_t = BandwidthDirection::Both)]
    pub direction: BandwidthDirection,

    /// 並列ストリーム数
    #[arg(short, long, default_value_t = 1)]
    pub parallel: usize,

    /// ブロックサイズ（バイト）
    #[arg(long, default_value_t = 131072)]
    pub block_size: usize,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

/// レイテンシ測定の引数
#[derive(Args, Debug)]
pub struct LatencyArgs {
    /// ターゲットアドレス
    #[arg(short, long)]
    pub target: SocketAddr,

    /// 測定時間（秒）
    #[arg(short, long, default_value_t = 60)]
    pub duration: u64,

    /// 測定間隔（ミリ秒）
    #[arg(short, long, default_value_t = 100)]
    pub interval: u64,

    /// ヒストグラム表示
    #[arg(long)]
    pub histogram: bool,

    /// タイムアウト（ミリ秒）
    #[arg(long, default_value_t = 5000)]
    pub timeout: u64,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bandwidth_direction_default() {
        let dir = BandwidthDirection::default();
        assert!(matches!(dir, BandwidthDirection::Both));
    }

    #[test]
    fn test_bandwidth_direction_up() {
        let dir = BandwidthDirection::Up;
        assert!(matches!(dir, BandwidthDirection::Up));
    }

    #[test]
    fn test_bandwidth_direction_down() {
        let dir = BandwidthDirection::Down;
        assert!(matches!(dir, BandwidthDirection::Down));
    }

    #[test]
    fn test_bandwidth_args_server_mode() {
        let args = BandwidthArgs {
            target: None,
            server: true,
            bind: "0.0.0.0:5201".parse().unwrap(),
            duration: 10,
            direction: BandwidthDirection::Both,
            parallel: 1,
            block_size: 131072,
            output: None,
        };
        assert!(args.server);
        assert!(args.target.is_none());
    }

    #[test]
    fn test_bandwidth_args_client_mode() {
        let args = BandwidthArgs {
            target: Some("192.168.1.100:5201".parse().unwrap()),
            server: false,
            bind: "0.0.0.0:5201".parse().unwrap(),
            duration: 30,
            direction: BandwidthDirection::Up,
            parallel: 4,
            block_size: 262144,
            output: Some("bandwidth.json".to_string()),
        };
        assert!(!args.server);
        assert!(args.target.is_some());
        assert_eq!(args.parallel, 4);
        assert!(args.output.is_some());
    }

    #[test]
    fn test_bandwidth_args_parallel() {
        let args = BandwidthArgs {
            target: Some("10.0.0.1:5201".parse().unwrap()),
            server: false,
            bind: "0.0.0.0:5201".parse().unwrap(),
            duration: 10,
            direction: BandwidthDirection::Both,
            parallel: 8,
            block_size: 131072,
            output: None,
        };
        assert_eq!(args.parallel, 8);
    }

    #[test]
    fn test_latency_args() {
        let args = LatencyArgs {
            target: "127.0.0.1:8080".parse().unwrap(),
            duration: 60,
            interval: 100,
            histogram: true,
            timeout: 5000,
            output: None,
        };
        assert!(args.histogram);
        assert_eq!(args.interval, 100);
    }

    #[test]
    fn test_latency_args_without_histogram() {
        let args = LatencyArgs {
            target: "10.0.0.1:9999".parse().unwrap(),
            duration: 30,
            interval: 50,
            histogram: false,
            timeout: 3000,
            output: Some("latency.json".to_string()),
        };
        assert!(!args.histogram);
        assert_eq!(args.duration, 30);
        assert!(args.output.is_some());
    }

    #[test]
    fn test_latency_args_custom_interval() {
        let args = LatencyArgs {
            target: "127.0.0.1:8080".parse().unwrap(),
            duration: 120,
            interval: 10,
            histogram: true,
            timeout: 1000,
            output: None,
        };
        assert_eq!(args.interval, 10);
        assert_eq!(args.timeout, 1000);
    }

    #[test]
    fn test_bench_commands_bandwidth() {
        let bw_args = BandwidthArgs {
            target: None,
            server: true,
            bind: "0.0.0.0:5201".parse().unwrap(),
            duration: 10,
            direction: BandwidthDirection::Both,
            parallel: 1,
            block_size: 131072,
            output: None,
        };
        let cmd = BenchCommands::Bandwidth(bw_args);
        assert!(matches!(cmd, BenchCommands::Bandwidth(_)));
    }

    #[test]
    fn test_bench_commands_latency() {
        let lat_args = LatencyArgs {
            target: "127.0.0.1:8080".parse().unwrap(),
            duration: 60,
            interval: 100,
            histogram: false,
            timeout: 5000,
            output: None,
        };
        let cmd = BenchCommands::Latency(lat_args);
        assert!(matches!(cmd, BenchCommands::Latency(_)));
    }
}
