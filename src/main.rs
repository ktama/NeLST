//! NeLST - Network Load and Security Test
//!
//! ネットワークの負荷テストとセキュリティテストを行うCLIツール。
//!
//! # 機能
//!
//! - **負荷テスト**: トラフィック負荷テスト、コネクション負荷テスト
//! - **セキュリティスキャン**: ポートスキャン（TCP Connect, SYN, FIN等）
//! - **テストサーバ**: エコーサーバ、シンクサーバ、フラッドサーバ
//!
//! # 使用例
//!
//! ```bash
//! # 負荷テスト
//! nelst load traffic -t 127.0.0.1:8080 -d 60
//!
//! # ポートスキャン
//! nelst scan port -t 192.168.1.1 --ports 1-1024
//!
//! # エコーサーバ起動
//! nelst server echo -b 0.0.0.0:8080
//! ```

mod cli;
mod common;
mod load;
mod scan;
mod server;

use clap::Parser;
use std::process::ExitCode;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use cli::{Cli, Commands};
use common::error::{format_error, ExitStatus, NelstError};
use common::output::Output;

fn main() -> ExitCode {
    // CLIをパース
    let cli = Cli::parse();

    // ロギングを初期化
    init_logging(cli.verbose);

    // 出力ハンドラを作成
    let output = Output::new(cli.json, cli.quiet);

    // コマンドを実行
    let result = run_command(&cli, &output);

    match result {
        Ok(_) => ExitCode::from(ExitStatus::Success),
        Err(e) => {
            if let Some(hint) = e.hint() {
                output.error_with_hint(&e.to_string(), hint);
            } else {
                output.error(&e.to_string());
            }
            error!("{}", format_error(&e));
            ExitCode::from(e.exit_status())
        }
    }
}

/// ロギングを初期化
///
/// ログレベルは以下の優先順位で決定される:
/// 1. RUST_LOG 環境変数
/// 2. --verbose フラグ（debug レベル）
/// 3. デフォルト（info レベル）
fn init_logging(verbose: bool) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if verbose {
            EnvFilter::new("debug")
        } else {
            EnvFilter::new("warn,nelst=info")
        }
    });

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).with_ansi(true))
        .with(filter)
        .init();
}

/// コマンドを実行
fn run_command(cli: &Cli, output: &Output) -> Result<(), NelstError> {
    // 設定ファイルを読み込み（オプション）
    let _config = common::config::Config::load(cli.config.as_deref())?;

    match &cli.command {
        Commands::Load { command } => run_load_command(command, output),
        Commands::Scan { command } => run_scan_command(command, output),
        Commands::Server { command } => run_server_command(command, output),
    }
}

/// 負荷テストコマンドを実行
fn run_load_command(command: &cli::load::LoadCommands, output: &Output) -> Result<(), NelstError> {
    use cli::load::LoadCommands;

    let rt = tokio::runtime::Runtime::new()?;

    match command {
        LoadCommands::Traffic(args) => {
            output.header("Network Load Test");
            output.info("Target", &args.target.to_string());
            output.info("Protocol", &format!("{:?}", args.protocol).to_lowercase());
            output.info("Mode", &format!("{:?}", args.mode).to_lowercase());
            output.info("Duration", &format!("{}s", args.duration));
            output.info("Concurrency", &args.concurrency.to_string());
            output.info("Packet Size", &format!("{} bytes", args.size));
            output.newline();

            info!("Starting traffic load test to {}", args.target);
            let result = rt.block_on(load::traffic::run(args))?;

            output.section("RESULTS");
            let _ = output.result(&result);
            Ok(())
        }
        LoadCommands::Connection(args) => {
            output.header("Connection Load Test");
            output.info("Target", &args.target.to_string());
            output.info("Count", &args.count.to_string());
            output.info("Concurrency", &args.concurrency.to_string());
            output.info("Timeout", &format!("{}ms", args.timeout));
            output.newline();

            info!("Starting connection load test to {}", args.target);
            let result = rt.block_on(load::connection::run(args))?;

            output.section("RESULTS");
            let _ = output.result(&result);
            Ok(())
        }
    }
}

/// スキャンコマンドを実行
fn run_scan_command(command: &cli::scan::ScanCommands, output: &Output) -> Result<(), NelstError> {
    use cli::scan::ScanCommands;

    let rt = tokio::runtime::Runtime::new()?;

    match command {
        ScanCommands::Port(args) => {
            output.header("Port Scanner");
            output.info("Target", &args.target.to_string());
            output.info("Method", &format!("{:?}", args.method));
            output.info("Ports", &args.ports);
            output.newline();

            info!("Starting port scan on {}", args.target);
            let result = rt.block_on(scan::tcp_connect::run(args))?;

            output.section("OPEN PORTS");
            let _ = output.result(&result);
            Ok(())
        }
    }
}

/// サーバコマンドを実行
fn run_server_command(
    command: &cli::server::ServerCommands,
    output: &Output,
) -> Result<(), NelstError> {
    use cli::server::ServerCommands;

    let rt = tokio::runtime::Runtime::new()?;

    match command {
        ServerCommands::Echo(args) => {
            output.header("Echo Server");
            output.info("Bind", &args.bind.to_string());
            output.info("Protocol", &format!("{:?}", args.protocol).to_lowercase());
            output.newline();
            output.message("Press Ctrl+C to stop the server.");
            output.newline();

            info!("Starting echo server on {}", args.bind);
            rt.block_on(server::echo::run(args))?;
            Ok(())
        }
        ServerCommands::Sink(args) => {
            output.header("Sink Server");
            output.info("Bind", &args.bind.to_string());
            output.info("Protocol", &format!("{:?}", args.protocol).to_lowercase());
            output.newline();
            output.message("Press Ctrl+C to stop the server.");
            output.newline();

            info!("Starting sink server on {}", args.bind);
            rt.block_on(server::sink::run(args))?;
            Ok(())
        }
        ServerCommands::Flood(args) => {
            output.header("Flood Server");
            output.info("Bind", &args.bind.to_string());
            output.info("Protocol", &format!("{:?}", args.protocol).to_lowercase());
            output.info("Size", &format!("{} bytes", args.size));
            output.newline();
            output.message("Press Ctrl+C to stop the server.");
            output.newline();

            info!("Starting flood server on {}", args.bind);
            // TODO: フェーズ1で実装
            todo!("Flood server not implemented yet")
        }
    }
}
