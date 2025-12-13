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
mod scan;
mod server;

use clap::Parser;
use serde::Serialize;
use std::fs;
use std::process::ExitCode;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use cli::{Cli, Commands};
use common::error::{ExitStatus, NelstError, format_error};
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

/// 結果をファイルに保存
fn save_result_to_file<T: Serialize>(result: &T, path: &str) -> Result<(), NelstError> {
    let json = serde_json::to_string_pretty(result)
        .map_err(|e| NelstError::config(format!("Failed to serialize result: {}", e)))?;
    fs::write(path, json)
        .map_err(|e| NelstError::config(format!("Failed to write to file '{}': {}", path, e)))?;
    info!("Results saved to {}", path);
    Ok(())
}

/// コマンドを実行
fn run_command(cli: &Cli, output: &Output) -> Result<(), NelstError> {
    // 設定ファイルを読み込み（オプション）
    let _config = common::config::Config::load(cli.config.as_deref())?;

    match &cli.command {
        Commands::Load { command } => run_load_command(command, output),
        Commands::Scan { command } => run_scan_command(command, output),
        Commands::Server { command } => run_server_command(command, output),
        Commands::Diag { command } => run_diag_command(command, output),
        Commands::Bench { command } => run_bench_command(command, output),
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

            // ファイル出力
            if let Some(ref path) = args.output {
                save_result_to_file(&result, path)?;
            }
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
            Ok(())
        }
    }
}

/// スキャンコマンドを実行
fn run_scan_command(command: &cli::scan::ScanCommands, output: &Output) -> Result<(), NelstError> {
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
fn run_diag_command(command: &cli::diag::DiagCommands, output: &Output) -> Result<(), NelstError> {
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
            Ok(())
        }
    }
}

/// ベンチマークコマンドを実行
fn run_bench_command(
    command: &cli::bench::BenchCommands,
    output: &Output,
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

            if args.histogram {
                if let Some(ref histogram) = result.histogram {
                    output.newline();
                    output.section("HISTOGRAM");
                    for line in bench::latency::format_histogram(histogram, 30) {
                        output.message(&line);
                    }
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
            Ok(())
        }
    }
}
