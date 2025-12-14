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

mod bench;
mod cli;
mod common;
mod diag;
mod load;
mod profile;
mod report;
mod scan;
mod server;

use clap::Parser;
use serde::Serialize;
use std::fs;
use std::process::ExitCode;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use cli::{Cli, Commands};
use common::error::{ExitStatus, NelstError, format_error};
use common::output::Output;
use profile::{Profile, ProfileManager};
use report::ReportFormat;

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

/// 結果をファイルに保存
fn save_result_to_file<T: Serialize>(result: &T, path: &str) -> Result<(), NelstError> {
    let json = serde_json::to_string_pretty(result)
        .map_err(|e| NelstError::config(format!("Failed to serialize result: {}", e)))?;
    fs::write(path, json)
        .map_err(|e| NelstError::config(format!("Failed to write to file '{}': {}", path, e)))?;
    info!("Results saved to {}", path);
    Ok(())
}

/// 結果を指定形式でファイルに保存
fn save_result_with_format<T: Serialize>(
    result: &T,
    path: &str,
    format: &ReportFormat,
) -> Result<(), NelstError> {
    let content = match format {
        ReportFormat::Json => serde_json::to_string_pretty(result)
            .map_err(|e| NelstError::config(format!("Failed to serialize to JSON: {}", e)))?,
        ReportFormat::Csv => {
            // CSVはテーブルデータ向けなので、JSONにフォールバック
            warn!("CSV format not suitable for this data, using JSON");
            serde_json::to_string_pretty(result)
                .map_err(|e| NelstError::config(format!("Failed to serialize: {}", e)))?
        }
        ReportFormat::Html => {
            let json = serde_json::to_string_pretty(result)
                .map_err(|e| NelstError::config(format!("Failed to serialize: {}", e)))?;
            format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>NeLST Report</title>
    <style>
        body {{ font-family: sans-serif; margin: 2em; }}
        pre {{ background: #f4f4f4; padding: 1em; overflow-x: auto; }}
    </style>
</head>
<body>
    <h1>NeLST Report</h1>
    <p>Generated: {}</p>
    <pre>{}</pre>
</body>
</html>"#,
                chrono::Utc::now().to_rfc3339(),
                json
            )
        }
        ReportFormat::Markdown => {
            let json = serde_json::to_string_pretty(result)
                .map_err(|e| NelstError::config(format!("Failed to serialize: {}", e)))?;
            format!(
                "# NeLST Report\n\n**Generated:** {}\n\n```json\n{}\n```\n",
                chrono::Utc::now().to_rfc3339(),
                json
            )
        }
        ReportFormat::Text => serde_json::to_string_pretty(result)
            .map_err(|e| NelstError::config(format!("Failed to serialize: {}", e)))?,
    };

    fs::write(path, content)
        .map_err(|e| NelstError::config(format!("Failed to write to file '{}': {}", path, e)))?;
    info!("Report saved to {} (format: {:?})", path, format);
    Ok(())
}

/// プロファイルから設定を読み込み、現在のオプションにマージする情報を表示
fn load_profile_info(profile_name: &str, output: &Output) -> Result<Profile, NelstError> {
    let manager = ProfileManager::new()?;
    let profile = manager.load(profile_name)?;
    output.message(&format!("Using profile: {}", profile.name));
    Ok(profile)
}

/// 現在のコマンドオプションをプロファイルとして保存
fn save_command_as_profile(
    cli: &Cli,
    profile_name: &str,
    output: &Output,
) -> Result<(), NelstError> {
    let manager = ProfileManager::new()?;

    let (command_type, subcommand_type, options) = match &cli.command {
        Commands::Load { command } => {
            let (sub, opts) = extract_load_options(command);
            ("load".to_string(), sub, opts)
        }
        Commands::Scan { command } => {
            let (sub, opts) = extract_scan_options(command);
            ("scan".to_string(), sub, opts)
        }
        Commands::Diag { command } => {
            let (sub, opts) = extract_diag_options(command);
            ("diag".to_string(), sub, opts)
        }
        Commands::Bench { command } => {
            let (sub, opts) = extract_bench_options(command);
            ("bench".to_string(), sub, opts)
        }
        Commands::Server { command } => {
            let (sub, opts) = extract_server_options(command);
            ("server".to_string(), sub, opts)
        }
        Commands::Profile { .. } => {
            return Err(NelstError::config(
                "Cannot save profile command as a profile",
            ));
        }
    };

    let mut profile = Profile::new(profile_name, &command_type, &subcommand_type, None);
    profile.options = options;

    manager.save(&profile)?;
    output.success(&format!("Profile '{}' saved.", profile_name));
    Ok(())
}

/// Loadコマンドのオプションを抽出
fn extract_load_options(
    command: &cli::load::LoadCommands,
) -> (String, std::collections::HashMap<String, serde_json::Value>) {
    use cli::load::LoadCommands;
    let mut opts = std::collections::HashMap::new();

    match command {
        LoadCommands::Traffic(args) => {
            opts.insert(
                "target".to_string(),
                serde_json::json!(args.target.to_string()),
            );
            opts.insert("duration".to_string(), serde_json::json!(args.duration));
            opts.insert(
                "concurrency".to_string(),
                serde_json::json!(args.concurrency),
            );
            opts.insert("size".to_string(), serde_json::json!(args.size));
            ("traffic".to_string(), opts)
        }
        LoadCommands::Connection(args) => {
            opts.insert(
                "target".to_string(),
                serde_json::json!(args.target.to_string()),
            );
            opts.insert("count".to_string(), serde_json::json!(args.count));
            opts.insert(
                "concurrency".to_string(),
                serde_json::json!(args.concurrency),
            );
            opts.insert("timeout".to_string(), serde_json::json!(args.timeout));
            ("connection".to_string(), opts)
        }
        LoadCommands::Http(args) => {
            opts.insert("url".to_string(), serde_json::json!(args.url));
            opts.insert("method".to_string(), serde_json::json!(args.method));
            opts.insert("duration".to_string(), serde_json::json!(args.duration));
            opts.insert(
                "concurrency".to_string(),
                serde_json::json!(args.concurrency),
            );
            ("http".to_string(), opts)
        }
    }
}

/// Scanコマンドのオプションを抽出
fn extract_scan_options(
    command: &cli::scan::ScanCommands,
) -> (String, std::collections::HashMap<String, serde_json::Value>) {
    use cli::scan::ScanCommands;
    let mut opts = std::collections::HashMap::new();

    match command {
        ScanCommands::Port(args) => {
            opts.insert(
                "target".to_string(),
                serde_json::json!(args.target.to_string()),
            );
            opts.insert("ports".to_string(), serde_json::json!(args.ports));
            opts.insert(
                "concurrency".to_string(),
                serde_json::json!(args.concurrency),
            );
            opts.insert("timeout".to_string(), serde_json::json!(args.timeout));
            ("port".to_string(), opts)
        }
    }
}

/// Diagコマンドのオプションを抽出
fn extract_diag_options(
    command: &cli::diag::DiagCommands,
) -> (String, std::collections::HashMap<String, serde_json::Value>) {
    use cli::diag::DiagCommands;
    let mut opts = std::collections::HashMap::new();

    match command {
        DiagCommands::Ping(args) => {
            opts.insert("target".to_string(), serde_json::json!(args.target));
            opts.insert("count".to_string(), serde_json::json!(args.count));
            opts.insert("interval".to_string(), serde_json::json!(args.interval));
            ("ping".to_string(), opts)
        }
        DiagCommands::Trace(args) => {
            opts.insert("target".to_string(), serde_json::json!(args.target));
            opts.insert("max_hops".to_string(), serde_json::json!(args.max_hops));
            ("trace".to_string(), opts)
        }
        DiagCommands::Dns(args) => {
            opts.insert("target".to_string(), serde_json::json!(args.target));
            opts.insert(
                "record_type".to_string(),
                serde_json::json!(format!("{:?}", args.record_type)),
            );
            ("dns".to_string(), opts)
        }
        DiagCommands::Mtu(args) => {
            opts.insert("target".to_string(), serde_json::json!(args.target));
            ("mtu".to_string(), opts)
        }
    }
}

/// Benchコマンドのオプションを抽出
fn extract_bench_options(
    command: &cli::bench::BenchCommands,
) -> (String, std::collections::HashMap<String, serde_json::Value>) {
    use cli::bench::BenchCommands;
    let mut opts = std::collections::HashMap::new();

    match command {
        BenchCommands::Bandwidth(args) => {
            if let Some(ref target) = args.target {
                opts.insert("target".to_string(), serde_json::json!(target.to_string()));
            }
            opts.insert("bind".to_string(), serde_json::json!(args.bind.to_string()));
            opts.insert("duration".to_string(), serde_json::json!(args.duration));
            ("bandwidth".to_string(), opts)
        }
        BenchCommands::Latency(args) => {
            opts.insert(
                "target".to_string(),
                serde_json::json!(args.target.to_string()),
            );
            opts.insert("duration".to_string(), serde_json::json!(args.duration));
            opts.insert("interval".to_string(), serde_json::json!(args.interval));
            ("latency".to_string(), opts)
        }
    }
}

/// Serverコマンドのオプションを抽出
fn extract_server_options(
    command: &cli::server::ServerCommands,
) -> (String, std::collections::HashMap<String, serde_json::Value>) {
    use cli::server::ServerCommands;
    let mut opts = std::collections::HashMap::new();

    match command {
        ServerCommands::Echo(args) => {
            opts.insert("bind".to_string(), serde_json::json!(args.bind.to_string()));
            ("echo".to_string(), opts)
        }
        ServerCommands::Sink(args) => {
            opts.insert("bind".to_string(), serde_json::json!(args.bind.to_string()));
            ("sink".to_string(), opts)
        }
        ServerCommands::Flood(args) => {
            opts.insert("bind".to_string(), serde_json::json!(args.bind.to_string()));
            opts.insert("size".to_string(), serde_json::json!(args.size));
            ("flood".to_string(), opts)
        }
        ServerCommands::Http(args) => {
            opts.insert("bind".to_string(), serde_json::json!(args.bind.to_string()));
            ("http".to_string(), opts)
        }
    }
}

/// コマンドを実行
fn run_command(cli: &Cli, output: &Output) -> Result<(), NelstError> {
    // 設定ファイルを読み込み（オプション）
    let _config = common::config::Config::load(cli.config.as_deref())?;

    // プロファイルが指定されている場合は情報を表示
    if let Some(ref profile_name) = cli.profile {
        let _profile = load_profile_info(profile_name, output)?;
        // TODO: プロファイルのオプションをマージする機能は将来実装
    }

    // コマンドを実行
    let result = match &cli.command {
        Commands::Load { command } => run_load_command(command, output, cli),
        Commands::Scan { command } => run_scan_command(command, output, cli),
        Commands::Server { command } => run_server_command(command, output),
        Commands::Diag { command } => run_diag_command(command, output, cli),
        Commands::Bench { command } => run_bench_command(command, output, cli),
        Commands::Profile { command } => run_profile_command(command, output),
    };

    // プロファイル保存が指定されている場合
    if let Some(ref profile_name) = cli.save_profile {
        save_command_as_profile(cli, profile_name, output)?;
    }

    result
}

/// 結果をレポートとして保存（--report と --format オプション対応）
fn save_report_if_requested<T: Serialize>(
    result: &T,
    cli: &Cli,
    output: &Output,
) -> Result<(), NelstError> {
    if let Some(ref report_path) = cli.report {
        let format = if let Some(ref fmt) = cli.format {
            ReportFormat::from_str(fmt)?
        } else {
            // 拡張子から推測
            if report_path.ends_with(".html") {
                ReportFormat::Html
            } else if report_path.ends_with(".md") {
                ReportFormat::Markdown
            } else if report_path.ends_with(".csv") {
                ReportFormat::Csv
            } else {
                ReportFormat::Json
            }
        };
        save_result_with_format(result, report_path, &format)?;
        output.success(&format!("Report saved to {}", report_path));
    }
    Ok(())
}

/// 負荷テストコマンドを実行
fn run_load_command(
    command: &cli::load::LoadCommands,
    output: &Output,
    cli: &Cli,
) -> Result<(), NelstError> {
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

            // ファイル出力
            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }

            // レポート出力
            save_report_if_requested(&result, cli, output)?;
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

            // ファイル出力
            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }

            // レポート出力
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
        LoadCommands::Http(args) => {
            output.header("HTTP Load Test");
            output.info("URL", &args.url);
            output.info("Method", &args.method);
            output.info("Duration", &format!("{}s", args.duration));
            output.info("Concurrency", &args.concurrency.to_string());
            if let Some(rate) = args.rate {
                output.info("Rate Limit", &format!("{} req/s", rate));
            }
            if !args.headers.is_empty() {
                output.info("Headers", &format!("{} custom", args.headers.len()));
            }
            if args.http2 {
                output.info("Protocol", "HTTP/2");
            }
            if args.insecure {
                output.info("TLS Verify", "disabled");
            }
            if args.follow_redirects {
                output.info("Redirects", "follow");
            }
            output.newline();

            info!("Starting HTTP load test to {}", args.url);
            let result = rt.block_on(load::http::run(args))?;

            output.section("RESULTS");
            let _ = output.result(&result);

            // ファイル出力
            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }

            // レポート出力
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
    }
}

/// スキャンコマンドを実行
fn run_scan_command(
    command: &cli::scan::ScanCommands,
    output: &Output,
    cli: &Cli,
) -> Result<(), NelstError> {
    use cli::scan::{ScanCommands, ScanMethod};

    let rt = tokio::runtime::Runtime::new()?;

    match command {
        ScanCommands::Port(args) => {
            output.header("Port Scanner");
            output.info("Target", &args.target.to_string());
            output.info("Method", &format!("{:?}", args.method));
            output.info("Ports", &args.ports);
            if args.grab_banner {
                output.info("Banner Grab", "enabled");
            }
            if args.ssl_check {
                output.info("SSL Check", "enabled");
            }
            output.newline();

            info!("Starting port scan on {}", args.target);

            // スキャン手法に応じて適切なモジュールを呼び出し
            let result = match args.method {
                ScanMethod::Tcp => rt.block_on(scan::tcp_connect::run(args))?,
                ScanMethod::Syn | ScanMethod::Fin | ScanMethod::Xmas | ScanMethod::Null => {
                    rt.block_on(scan::syn::run(args))?
                }
                ScanMethod::Udp => rt.block_on(scan::udp::run(args))?,
            };

            output.section("OPEN PORTS");
            let _ = output.result(&result);

            // サービス検出（オプション）
            if args.service_detection || args.grab_banner {
                let open_ports: Vec<u16> = result
                    .ports
                    .iter()
                    .filter(|p| matches!(p.state, scan::tcp_connect::PortState::Open))
                    .map(|p| p.port)
                    .collect();

                if !open_ports.is_empty() {
                    output.section("SERVICE DETECTION");
                    let services = rt.block_on(scan::service::detect_services(
                        args.target,
                        &open_ports,
                        args.timeout,
                        args.grab_banner,
                        args.concurrency,
                    ));

                    for service in &services {
                        let mut info_parts = Vec::new();
                        if let Some(ref name) = service.name {
                            info_parts.push(name.clone());
                        }
                        if let Some(ref product) = service.product {
                            info_parts.push(product.clone());
                        }
                        if let Some(ref version) = service.version {
                            info_parts.push(format!("({})", version));
                        }
                        if let Some(ref banner) = service.banner {
                            let short_banner: String = banner.chars().take(50).collect();
                            info_parts.push(format!("[{}...]", short_banner));
                        }

                        output.info(&format!("Port {}", service.port), &info_parts.join(" "));
                    }
                    output.newline();
                }
            }

            // SSL/TLS検査（オプション）
            if args.ssl_check {
                let ssl_ports: Vec<u16> = result
                    .ports
                    .iter()
                    .filter(|p| {
                        matches!(p.state, scan::tcp_connect::PortState::Open)
                            && is_likely_ssl_port(p.port)
                    })
                    .map(|p| p.port)
                    .collect();

                if !ssl_ports.is_empty() {
                    output.section("SSL/TLS INSPECTION");
                    let hostname = args.hostname.as_deref().unwrap_or(
                        // IPアドレスをホスト名として使用
                        "localhost",
                    );

                    let ssl_results = rt.block_on(scan::ssl::inspect_ssl_ports(
                        args.target,
                        &ssl_ports,
                        hostname,
                        args.timeout,
                        args.concurrency,
                    ));

                    for ssl_info in &ssl_results {
                        if ssl_info.is_valid {
                            if let Some(ref tls_version) = ssl_info.tls_version {
                                output.info(&format!("Port {}", ssl_info.port), tls_version);
                            }
                            if let Some(ref cert) = ssl_info.certificate {
                                output.info("  Subject", &cert.subject);
                                output.info("  Issuer", &cert.issuer);
                                output.info(
                                    "  Expiry",
                                    &format!(
                                        "{} ({}d)",
                                        if cert.is_expired { "EXPIRED" } else { "Valid" },
                                        cert.days_until_expiry
                                    ),
                                );
                            }
                        } else {
                            let errors = ssl_info.errors.join(", ");
                            output.info(
                                &format!("Port {}", ssl_info.port),
                                &format!("Error: {}", errors),
                            );
                        }
                    }
                    output.newline();
                }
            }

            // ファイル出力
            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }

            // レポート出力
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
    }
}

/// SSL/TLSが使用されている可能性が高いポートか判定
fn is_likely_ssl_port(port: u16) -> bool {
    matches!(port, 443 | 465 | 636 | 853 | 993 | 995 | 8443 | 9443)
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
            rt.block_on(server::flood::run(args))?;
            Ok(())
        }
        ServerCommands::Http(args) => {
            output.header("HTTP Test Server");
            output.info("Bind", &format!("http://{}", args.bind));
            output.info("Status", &args.status.to_string());
            if args.delay > 0 {
                output.info("Delay", &format!("{}ms", args.delay));
            }
            if args.error_rate > 0.0 {
                output.info("Error Rate", &format!("{:.1}%", args.error_rate * 100.0));
            }
            output.newline();
            output.message("Press Ctrl+C to stop the server.");
            output.newline();

            info!("Starting HTTP server on {}", args.bind);
            rt.block_on(server::http::run(args))?;
            Ok(())
        }
    }
}

/// 診断コマンドを実行
fn run_diag_command(
    command: &cli::diag::DiagCommands,
    output: &Output,
    cli: &Cli,
) -> Result<(), NelstError> {
    use cli::diag::DiagCommands;

    let rt = tokio::runtime::Runtime::new()?;

    match command {
        DiagCommands::Ping(args) => {
            let mode = if args.tcp { "TCP" } else { "ICMP" };
            output.header("Ping Test");
            output.info("Target", &args.target);
            output.info("Mode", mode);
            output.info("Count", &args.count.to_string());
            output.info("Interval", &format!("{}ms", args.interval));
            output.info("Timeout", &format!("{}ms", args.timeout));
            output.newline();

            let result = rt.block_on(diag::ping::run(args))?;

            output.section("RESULTS");
            output.info("Transmitted", &result.transmitted.to_string());
            output.info("Received", &result.received.to_string());
            output.info("Packet Loss", &format!("{:.1}%", result.packet_loss));
            output.newline();

            if result.received > 0 {
                output.info("Min RTT", &format!("{:.3} ms", result.min_rtt));
                output.info("Avg RTT", &format!("{:.3} ms", result.avg_rtt));
                output.info("Max RTT", &format!("{:.3} ms", result.max_rtt));
                output.info("Stddev", &format!("{:.3} ms", result.stddev_rtt));
            }

            output.json(&result)?;

            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
        DiagCommands::Trace(args) => {
            output.header("Traceroute");
            output.info("Target", &args.target);
            output.info("Mode", &format!("{:?}", args.mode));
            output.info("Max Hops", &args.max_hops.to_string());
            output.newline();

            let result = rt.block_on(diag::trace::run(args))?;

            output.section("ROUTE");
            for hop in &result.hops {
                let addr = hop.address.as_deref().unwrap_or("*");
                let rtts: Vec<String> = hop
                    .rtts
                    .iter()
                    .map(|r| match r {
                        Some(rtt) => format!("{:.2}ms", rtt),
                        None => "*".to_string(),
                    })
                    .collect();
                output.info(
                    &format!("{:>2}", hop.ttl),
                    &format!("{:<20} {}", addr, rtts.join("  ")),
                );
            }

            output.newline();
            if result.reached_destination {
                output.success(&format!(
                    "Reached destination in {} hops",
                    result.total_hops
                ));
            } else {
                output.warning("Did not reach destination");
            }

            output.json(&result)?;

            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
        DiagCommands::Dns(args) => {
            output.header("DNS Lookup");
            output.info("Query", &args.target);
            output.info("Type", &format!("{:?}", args.record_type));
            if let Some(server) = args.server {
                output.info("Server", &server.to_string());
            }
            output.info("Protocol", if args.tcp { "TCP" } else { "UDP" });
            output.newline();

            let result = rt.block_on(diag::dns::run(args))?;

            output.section("RECORDS");
            if result.records.is_empty() {
                output.warning("No records found");
                if let Some(ref err) = result.error {
                    output.error(err);
                }
            } else {
                for record in &result.records {
                    output.info(
                        &record.record_type,
                        &format!("{} (TTL: {})", record.value, record.ttl),
                    );
                }
            }
            output.newline();
            output.info("Query Time", &format!("{:.2} ms", result.resolve_time_ms));

            output.json(&result)?;

            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
        DiagCommands::Mtu(args) => {
            output.header("MTU Discovery");
            output.info("Target", &args.target);
            output.info(
                "Range",
                &format!("{} - {} bytes", args.min_mtu, args.max_mtu),
            );
            output.newline();

            let result = rt.block_on(diag::mtu::run(args))?;

            output.section("RESULT");
            output.info("Path MTU", &format!("{} bytes", result.path_mtu));
            output.info(
                "Discovery Time",
                &format!("{:.2} ms", result.discovery_time_ms),
            );

            output.json(&result)?;

            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
    }
}

/// ベンチマークコマンドを実行
fn run_bench_command(
    command: &cli::bench::BenchCommands,
    output: &Output,
    cli: &Cli,
) -> Result<(), NelstError> {
    use cli::bench::BenchCommands;

    let rt = tokio::runtime::Runtime::new()?;

    match command {
        BenchCommands::Bandwidth(args) => {
            if args.server {
                output.header("Bandwidth Server");
                output.info("Bind", &args.bind.to_string());
                output.newline();
                output.message("Press Ctrl+C to stop the server.");
                output.newline();

                rt.block_on(bench::bandwidth::run(args))?;
            } else {
                output.header("Bandwidth Test");
                if let Some(target) = args.target {
                    output.info("Target", &target.to_string());
                }
                output.info("Duration", &format!("{}s", args.duration));
                output.info("Direction", &format!("{:?}", args.direction));
                output.info("Parallel", &args.parallel.to_string());
                output.newline();

                let result = rt.block_on(bench::bandwidth::run(args))?;

                output.section("RESULTS");
                if let Some(ref upload) = result.upload {
                    output.info(
                        "Upload",
                        &format!(
                            "{:.2} Mbps (peak: {:.2} Mbps)",
                            upload.bandwidth_mbps, upload.peak_mbps
                        ),
                    );
                }
                if let Some(ref download) = result.download {
                    output.info(
                        "Download",
                        &format!(
                            "{:.2} Mbps (peak: {:.2} Mbps)",
                            download.bandwidth_mbps, download.peak_mbps
                        ),
                    );
                }

                output.json(&result)?;

                if let Some(ref path) = args.output {
                    save_result_to_file(&result, path)?;
                }
                save_report_if_requested(&result, cli, output)?;
            }
            Ok(())
        }
        BenchCommands::Latency(args) => {
            output.header("Latency Measurement");
            output.info("Target", &args.target.to_string());
            output.info("Duration", &format!("{}s", args.duration));
            output.info("Interval", &format!("{}ms", args.interval));
            output.newline();

            let result = rt.block_on(bench::latency::run(args))?;

            output.section("RESULTS");
            output.info(
                "Measurements",
                &format!("{} ({} successful)", result.count, result.success_count),
            );
            output.info("Success Rate", &format!("{:.1}%", result.success_rate));
            output.newline();

            output.info("Min", &format!("{:.2} ms", result.min_ms));
            output.info("Avg", &format!("{:.2} ms", result.avg_ms));
            output.info("Max", &format!("{:.2} ms", result.max_ms));
            output.info("Stddev", &format!("{:.2} ms", result.stddev_ms));
            output.newline();

            output.info("P50", &format!("{:.2} ms", result.p50_ms));
            output.info("P95", &format!("{:.2} ms", result.p95_ms));
            output.info("P99", &format!("{:.2} ms", result.p99_ms));

            if args.histogram
                && let Some(ref histogram) = result.histogram
            {
                output.newline();
                output.section("HISTOGRAM");
                for line in bench::latency::format_histogram(histogram, 30) {
                    output.message(&line);
                }
            }

            if !result.outliers.is_empty() {
                output.newline();
                output.warning(&format!("Detected {} outliers", result.outliers.len()));
            }

            output.json(&result)?;

            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }
            save_report_if_requested(&result, cli, output)?;
            Ok(())
        }
    }
}

/// プロファイル管理コマンドを実行
fn run_profile_command(
    command: &cli::profile::ProfileCommands,
    output: &Output,
) -> Result<(), NelstError> {
    use cli::profile::ProfileCommands;
    use profile::ProfileManager;

    let manager = ProfileManager::new()?;

    match command {
        ProfileCommands::List => {
            output.header("Saved Profiles");

            let profiles = manager.list()?;
            if profiles.is_empty() {
                output.message("No profiles saved.");
                output.message("");
                output.message("To save a profile, use --save-profile option with any command.");
            } else {
                output.newline();
                for p in &profiles {
                    output.info(
                        &p.name,
                        &format!(
                            "{} {} - {}",
                            p.command_type, p.subcommand_type, p.description
                        ),
                    );
                }
                output.newline();
                output.message(&format!("Total: {} profile(s)", profiles.len()));
            }

            if output.is_json() {
                output.json(&profiles)?;
            }
            Ok(())
        }
        ProfileCommands::Show(args) => {
            let profile = manager.load(&args.name)?;

            output.header(&format!("Profile: {}", profile.name));
            output.info(
                "Type",
                &format!("{} {}", profile.command_type, profile.subcommand_type),
            );
            output.info("Description", &profile.description);
            output.info("Created", &profile.created_at);
            output.info("Updated", &profile.updated_at);
            output.newline();

            output.section("OPTIONS");
            for (key, value) in &profile.options {
                output.info(key, &value.to_string());
            }

            output.json(&profile)?;
            Ok(())
        }
        ProfileCommands::Delete(args) => {
            if !manager.exists(&args.name) {
                return Err(NelstError::config(format!(
                    "Profile '{}' not found",
                    args.name
                )));
            }

            manager.delete(&args.name)?;
            output.success(&format!("Profile '{}' deleted.", args.name));
            Ok(())
        }
        ProfileCommands::Export(args) => {
            let output_path = args
                .output
                .clone()
                .unwrap_or_else(|| format!("{}.toml", args.name));

            manager.export(&args.name, &output_path)?;
            output.success(&format!(
                "Profile '{}' exported to '{}'",
                args.name, output_path
            ));
            Ok(())
        }
        ProfileCommands::Import(args) => {
            let profile = manager.import(&args.file, args.name.as_deref())?;
            output.success(&format!(
                "Profile '{}' imported successfully.",
                profile.name
            ));
            Ok(())
        }
    }
}
