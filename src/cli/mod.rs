//! CLI引数パーサーモジュール
//!
//! clapを使用したコマンドライン引数の解析を行う。

#![allow(dead_code)]

use clap::{Parser, Subcommand};

pub mod bench;
pub mod diag;
pub mod load;
pub mod profile;
pub mod scan;
pub mod server;

/// NeLST - Network Load and Security Test
///
/// ネットワークの負荷テストとセキュリティテストを行うCLIツール
#[derive(Parser, Debug)]
#[command(name = "nelst")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 詳細ログを出力
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// 出力を最小限に抑える
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// JSON形式で出力
    #[arg(long, global = true)]
    pub json: bool,

    /// 設定ファイルを指定
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<String>,

    /// 現在のオプションをプロファイルとして保存
    #[arg(long, global = true, value_name = "NAME")]
    pub save_profile: Option<String>,

    /// 保存済みプロファイルを使用
    #[arg(long, global = true, value_name = "NAME")]
    pub profile: Option<String>,

    /// 出力形式を指定（json, csv, html, markdown, text）
    #[arg(long, global = true, value_name = "FORMAT")]
    pub format: Option<String>,

    /// レポートをファイルに保存
    #[arg(long, global = true, value_name = "FILE")]
    pub report: Option<String>,
}

/// 利用可能なコマンド
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 負荷テスト（トラフィック/コネクション/HTTP）
    Load {
        #[command(subcommand)]
        command: load::LoadCommands,
    },

    /// セキュリティスキャン（ポート/SSL/TLS）
    Scan {
        #[command(subcommand)]
        command: scan::ScanCommands,
    },

    /// テスト用サーバを起動
    Server {
        #[command(subcommand)]
        command: server::ServerCommands,
    },

    /// ネットワーク診断（ping/traceroute/DNS）
    Diag {
        #[command(subcommand)]
        command: diag::DiagCommands,
    },

    /// 帯域幅・レイテンシ測定
    Bench {
        #[command(subcommand)]
        command: bench::BenchCommands,
    },

    /// プロファイル管理
    Profile {
        #[command(subcommand)]
        command: profile::ProfileCommands,
    },
}

/// CLIをパースして返す
///
/// コマンドライン引数を解析し、`Cli`構造体を返す。
/// 引数が不正な場合は自動的にヘルプメッセージを表示して終了する。
#[inline]
pub fn parse() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_cli_global_options() {
        // --verbose オプション
        let cli = Cli::try_parse_from(["nelst", "-v", "profile", "list"]).unwrap();
        assert!(cli.verbose);
        assert!(!cli.quiet);
        assert!(!cli.json);

        // --quiet オプション
        let cli = Cli::try_parse_from(["nelst", "-q", "profile", "list"]).unwrap();
        assert!(cli.quiet);
        assert!(!cli.verbose);

        // --json オプション
        let cli = Cli::try_parse_from(["nelst", "--json", "profile", "list"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn test_cli_config_option() {
        let cli = Cli::try_parse_from([
            "nelst",
            "--config",
            "/path/to/config.toml",
            "profile",
            "list",
        ])
        .unwrap();
        assert_eq!(cli.config, Some("/path/to/config.toml".to_string()));
    }

    #[test]
    fn test_cli_save_profile_option() {
        let cli = Cli::try_parse_from([
            "nelst",
            "--save-profile",
            "my-profile",
            "diag",
            "ping",
            "-t",
            "example.com",
        ])
        .unwrap();
        assert_eq!(cli.save_profile, Some("my-profile".to_string()));
    }

    #[test]
    fn test_cli_profile_option() {
        let cli = Cli::try_parse_from([
            "nelst",
            "--profile",
            "saved-profile",
            "diag",
            "ping",
            "-t",
            "example.com",
        ])
        .unwrap();
        assert_eq!(cli.profile, Some("saved-profile".to_string()));
    }

    #[test]
    fn test_cli_format_option() {
        let cli = Cli::try_parse_from([
            "nelst",
            "--format",
            "html",
            "diag",
            "ping",
            "-t",
            "example.com",
        ])
        .unwrap();
        assert_eq!(cli.format, Some("html".to_string()));
    }

    #[test]
    fn test_cli_report_option() {
        let cli = Cli::try_parse_from([
            "nelst",
            "--report",
            "output.html",
            "diag",
            "ping",
            "-t",
            "example.com",
        ])
        .unwrap();
        assert_eq!(cli.report, Some("output.html".to_string()));
    }

    #[test]
    fn test_cli_combined_options() {
        let cli = Cli::try_parse_from([
            "nelst",
            "-v",
            "--json",
            "--config",
            "my.toml",
            "--format",
            "markdown",
            "--report",
            "report.md",
            "--save-profile",
            "test-profile",
            "profile",
            "list",
        ])
        .unwrap();
        assert!(cli.verbose);
        assert!(cli.json);
        assert_eq!(cli.config, Some("my.toml".to_string()));
        assert_eq!(cli.format, Some("markdown".to_string()));
        assert_eq!(cli.report, Some("report.md".to_string()));
        assert_eq!(cli.save_profile, Some("test-profile".to_string()));
    }

    #[test]
    fn test_cli_commands_parsing() {
        // Load command
        let cli =
            Cli::try_parse_from(["nelst", "load", "traffic", "-t", "127.0.0.1:8080"]).unwrap();
        assert!(matches!(cli.command, Commands::Load { .. }));

        // Scan command
        let cli = Cli::try_parse_from(["nelst", "scan", "port", "-t", "192.168.1.1"]).unwrap();
        assert!(matches!(cli.command, Commands::Scan { .. }));

        // Server command
        let cli = Cli::try_parse_from(["nelst", "server", "echo"]).unwrap();
        assert!(matches!(cli.command, Commands::Server { .. }));

        // Diag command
        let cli = Cli::try_parse_from(["nelst", "diag", "ping", "-t", "example.com"]).unwrap();
        assert!(matches!(cli.command, Commands::Diag { .. }));

        // Bench command
        let cli = Cli::try_parse_from(["nelst", "bench", "bandwidth", "--server"]).unwrap();
        assert!(matches!(cli.command, Commands::Bench { .. }));

        // Profile command
        let cli = Cli::try_parse_from(["nelst", "profile", "list"]).unwrap();
        assert!(matches!(cli.command, Commands::Profile { .. }));
    }
}
