//! ネットワーク診断サブコマンドの定義

use clap::{Args, Subcommand, ValueEnum};
use std::net::IpAddr;

/// ネットワーク診断のサブコマンド
#[derive(Subcommand, Debug)]
pub enum DiagCommands {
    /// ICMP/TCP pingテスト
    Ping(PingArgs),
    /// 経路追跡（traceroute）
    Trace(TraceArgs),
    /// DNS解決テスト
    Dns(DnsArgs),
    /// MTU探索
    Mtu(MtuArgs),
}

/// Pingの引数
#[derive(Args, Debug)]
pub struct PingArgs {
    /// ターゲットホスト（IPアドレスまたはホスト名）
    #[arg(short, long)]
    pub target: String,

    /// 送信回数
    #[arg(short, long, default_value_t = 4)]
    pub count: u32,

    /// 送信間隔（ミリ秒）
    #[arg(short, long, default_value_t = 1000)]
    pub interval: u64,

    /// タイムアウト（ミリ秒）
    #[arg(long, default_value_t = 5000)]
    pub timeout: u64,

    /// TCP pingを使用（ICMP不可時）
    #[arg(long)]
    pub tcp: bool,

    /// TCPポート（--tcp使用時）
    #[arg(long, default_value_t = 80)]
    pub port: u16,

    /// パケットサイズ（バイト）
    #[arg(short, long, default_value_t = 64)]
    pub size: usize,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

/// Tracerouteモード
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum TraceMode {
    /// UDP（デフォルト）
    #[default]
    Udp,
    /// TCP
    Tcp,
    /// ICMP
    Icmp,
}

/// Tracerouteの引数
#[derive(Args, Debug)]
pub struct TraceArgs {
    /// ターゲットホスト（IPアドレスまたはホスト名）
    #[arg(short, long)]
    pub target: String,

    /// 最大ホップ数
    #[arg(long, default_value_t = 30)]
    pub max_hops: u8,

    /// トレースモード
    #[arg(short, long, value_enum, default_value_t = TraceMode::Udp)]
    pub mode: TraceMode,

    /// 各ホップでの試行回数
    #[arg(long, default_value_t = 3)]
    pub queries: u8,

    /// タイムアウト（ミリ秒）
    #[arg(long, default_value_t = 5000)]
    pub timeout: u64,

    /// ポート（TCP/UDPモード時）
    #[arg(short, long, default_value_t = 33434)]
    pub port: u16,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

/// DNSレコードタイプ
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum DnsRecordType {
    /// A（IPv4アドレス）
    #[default]
    A,
    /// AAAA（IPv6アドレス）
    Aaaa,
    /// MX（メールサーバ）
    Mx,
    /// TXT（テキストレコード）
    Txt,
    /// NS（ネームサーバ）
    Ns,
    /// CNAME（別名）
    Cname,
    /// SOA（権威情報）
    Soa,
    /// PTR（逆引き）
    Ptr,
    /// すべて
    All,
}

/// DNS解決の引数
#[derive(Args, Debug)]
pub struct DnsArgs {
    /// 対象ドメイン
    #[arg(short, long)]
    pub target: String,

    /// レコードタイプ
    #[arg(long, value_enum, default_value_t = DnsRecordType::A)]
    pub record_type: DnsRecordType,

    /// DNSサーバ指定
    #[arg(short, long)]
    pub server: Option<IpAddr>,

    /// TCP経由で問い合わせ
    #[arg(long)]
    pub tcp: bool,

    /// タイムアウト（ミリ秒）
    #[arg(long, default_value_t = 5000)]
    pub timeout: u64,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

/// MTU探索の引数
#[derive(Args, Debug)]
pub struct MtuArgs {
    /// ターゲットホスト
    #[arg(short, long)]
    pub target: String,

    /// 最小MTU
    #[arg(long, default_value_t = 68)]
    pub min_mtu: u16,

    /// 最大MTU
    #[arg(long, default_value_t = 1500)]
    pub max_mtu: u16,

    /// タイムアウト（ミリ秒）
    #[arg(long, default_value_t = 3000)]
    pub timeout: u64,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_args_defaults() {
        let args = PingArgs {
            target: "127.0.0.1".to_string(),
            count: 4,
            interval: 1000,
            timeout: 5000,
            tcp: false,
            port: 80,
            size: 64,
            output: None,
        };
        assert_eq!(args.count, 4);
        assert_eq!(args.interval, 1000);
    }

    #[test]
    fn test_ping_args_tcp_mode() {
        let args = PingArgs {
            target: "example.com".to_string(),
            count: 10,
            interval: 500,
            timeout: 3000,
            tcp: true,
            port: 443,
            size: 128,
            output: Some("output.json".to_string()),
        };
        assert!(args.tcp);
        assert_eq!(args.port, 443);
        assert!(args.output.is_some());
    }

    #[test]
    fn test_trace_mode_default() {
        let mode = TraceMode::default();
        assert!(matches!(mode, TraceMode::Udp));
    }

    #[test]
    fn test_trace_mode_tcp() {
        let mode = TraceMode::Tcp;
        assert!(matches!(mode, TraceMode::Tcp));
    }

    #[test]
    fn test_trace_mode_icmp() {
        let mode = TraceMode::Icmp;
        assert!(matches!(mode, TraceMode::Icmp));
    }

    #[test]
    fn test_trace_args() {
        let args = TraceArgs {
            target: "8.8.8.8".to_string(),
            max_hops: 15,
            mode: TraceMode::Tcp,
            queries: 5,
            timeout: 3000,
            port: 443,
            output: None,
        };
        assert_eq!(args.max_hops, 15);
        assert_eq!(args.queries, 5);
        assert!(matches!(args.mode, TraceMode::Tcp));
    }

    #[test]
    fn test_dns_record_type_default() {
        let record_type = DnsRecordType::default();
        assert!(matches!(record_type, DnsRecordType::A));
    }

    #[test]
    fn test_dns_record_types() {
        assert!(matches!(DnsRecordType::A, DnsRecordType::A));
        assert!(matches!(DnsRecordType::Aaaa, DnsRecordType::Aaaa));
        assert!(matches!(DnsRecordType::Mx, DnsRecordType::Mx));
        assert!(matches!(DnsRecordType::Txt, DnsRecordType::Txt));
        assert!(matches!(DnsRecordType::Ns, DnsRecordType::Ns));
        assert!(matches!(DnsRecordType::Cname, DnsRecordType::Cname));
        assert!(matches!(DnsRecordType::Soa, DnsRecordType::Soa));
        assert!(matches!(DnsRecordType::Ptr, DnsRecordType::Ptr));
        assert!(matches!(DnsRecordType::All, DnsRecordType::All));
    }

    #[test]
    fn test_dns_args() {
        let server: IpAddr = "8.8.8.8".parse().unwrap();
        let args = DnsArgs {
            target: "example.com".to_string(),
            record_type: DnsRecordType::Aaaa,
            server: Some(server),
            tcp: true,
            timeout: 3000,
            output: None,
        };
        assert!(args.tcp);
        assert!(args.server.is_some());
        assert!(matches!(args.record_type, DnsRecordType::Aaaa));
    }

    #[test]
    fn test_mtu_args() {
        let args = MtuArgs {
            target: "192.168.1.1".to_string(),
            min_mtu: 100,
            max_mtu: 9000,
            timeout: 2000,
            output: Some("mtu.json".to_string()),
        };
        assert_eq!(args.min_mtu, 100);
        assert_eq!(args.max_mtu, 9000);
        assert!(args.output.is_some());
    }

    #[test]
    fn test_mtu_args_defaults() {
        let args = MtuArgs {
            target: "10.0.0.1".to_string(),
            min_mtu: 68,
            max_mtu: 1500,
            timeout: 3000,
            output: None,
        };
        assert_eq!(args.min_mtu, 68);
        assert_eq!(args.max_mtu, 1500);
    }

    #[test]
    fn test_diag_commands_ping() {
        let ping_args = PingArgs {
            target: "localhost".to_string(),
            count: 4,
            interval: 1000,
            timeout: 5000,
            tcp: false,
            port: 80,
            size: 64,
            output: None,
        };
        let cmd = DiagCommands::Ping(ping_args);
        assert!(matches!(cmd, DiagCommands::Ping(_)));
    }

    #[test]
    fn test_diag_commands_trace() {
        let trace_args = TraceArgs {
            target: "google.com".to_string(),
            max_hops: 30,
            mode: TraceMode::Udp,
            queries: 3,
            timeout: 5000,
            port: 33434,
            output: None,
        };
        let cmd = DiagCommands::Trace(trace_args);
        assert!(matches!(cmd, DiagCommands::Trace(_)));
    }

    #[test]
    fn test_diag_commands_dns() {
        let dns_args = DnsArgs {
            target: "example.org".to_string(),
            record_type: DnsRecordType::Mx,
            server: None,
            tcp: false,
            timeout: 5000,
            output: None,
        };
        let cmd = DiagCommands::Dns(dns_args);
        assert!(matches!(cmd, DiagCommands::Dns(_)));
    }

    #[test]
    fn test_diag_commands_mtu() {
        let mtu_args = MtuArgs {
            target: "1.1.1.1".to_string(),
            min_mtu: 68,
            max_mtu: 1500,
            timeout: 3000,
            output: None,
        };
        let cmd = DiagCommands::Mtu(mtu_args);
        assert!(matches!(cmd, DiagCommands::Mtu(_)));
    }
}
