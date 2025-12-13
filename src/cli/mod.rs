//! CLI引数パーサーモジュール
//!
//! clapを使用したコマンドライン引数の解析を行う。

#![allow(dead_code)]

use clap::{Parser, Subcommand};

pub mod bench;
pub mod diag;
pub mod load;
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
}
