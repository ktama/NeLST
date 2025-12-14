//! HTTP負荷テストモジュール
//!
//! HTTPリクエストを送信し続けてサーバの負荷テストを行う。

use crate::cli::load::HttpArgs;
use crate::common::error::{NelstError, Result};
use crate::common::output::create_duration_progress_bar;
use crate::common::stats::{LatencyCollector, LoadTestResult, Timer};
use reqwest::{Client, Method, Response};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::debug;

/// HTTP負荷テストを実行
pub async fn run(args: &HttpArgs) -> Result<LoadTestResult> {
    let test = HttpTest::new(args)?;
    test.run().await
}

/// HTTP負荷テストの実行コンテキスト
#[derive(Debug)]
pub struct HttpTest {
    url: String,
    method: Method,
    headers: HashMap<String, String>,
    body: Option<String>,
    duration_secs: u64,
    concurrency: usize,
    rate: Option<u64>,
    insecure: bool,
    follow_redirects: bool,
    timeout: Duration,
    http2: bool,
}

impl HttpTest {
    /// 新しいテストを作成
    pub fn new(args: &HttpArgs) -> Result<Self> {
        // メソッドをパース
        let method = match args.method.to_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "DELETE" => Method::DELETE,
            "PATCH" => Method::PATCH,
            "HEAD" => Method::HEAD,
            "OPTIONS" => Method::OPTIONS,
            other => {
                return Err(NelstError::argument(format!(
                    "Unknown HTTP method: {}",
                    other
                )));
            }
        };

        // ヘッダーをパース
        let mut headers = HashMap::new();
        for h in &args.headers {
            if let Some((key, value)) = h.split_once(':') {
                headers.insert(key.trim().to_string(), value.trim().to_string());
            } else {
                return Err(NelstError::argument(format!(
                    "Invalid header format: {}. Use 'Key: Value'",
                    h
                )));
            }
        }

        // ボディを取得
        let body = if let Some(ref b) = args.body {
            if let Some(path) = b.strip_prefix('@') {
                // ファイルから読み込み
                Some(fs::read_to_string(path).map_err(|e| {
                    NelstError::config(format!("Failed to read body file '{}': {}", path, e))
                })?)
            } else {
                Some(b.clone())
            }
        } else {
            None
        };

        Ok(Self {
            url: args.url.clone(),
            method,
            headers,
            body,
            duration_secs: args.duration,
            concurrency: args.concurrency,
            rate: args.rate,
            insecure: args.insecure,
            follow_redirects: args.follow_redirects,
            timeout: Duration::from_millis(args.timeout),
            http2: args.http2,
        })
    }

    /// クライアントを作成（接続プール最適化済み）
    fn build_client(&self) -> Result<Client> {
        let mut builder = Client::builder()
            .timeout(self.timeout)
            .danger_accept_invalid_certs(self.insecure)
            // 接続プールの最適化
            .pool_max_idle_per_host(self.concurrency.max(10))
            .pool_idle_timeout(Duration::from_secs(30))
            // TCP_NODELAY を有効にして遅延を削減
            .tcp_nodelay(true)
            // Keep-Alive を有効化
            .tcp_keepalive(Duration::from_secs(60));

        if !self.follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        // HTTP/2設定
        if self.http2 {
            builder = builder.http2_prior_knowledge();
        }

        builder
            .build()
            .map_err(|e| NelstError::config(format!("Failed to build HTTP client: {}", e)))
    }

    /// テストを実行
    pub async fn run(&self) -> Result<LoadTestResult> {
        let timer = Timer::new();
        let running = Arc::new(AtomicBool::new(true));

        // 共有カウンター
        let total = Arc::new(AtomicU64::new(0));
        let success = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));
        let bytes_sent = Arc::new(AtomicU64::new(0));
        let bytes_received = Arc::new(AtomicU64::new(0));
        let latencies = Arc::new(Mutex::new(LatencyCollector::new()));
        let status_codes = Arc::new(Mutex::new(HashMap::<u16, u64>::new()));

        // プログレスバー
        let pb = create_duration_progress_bar(self.duration_secs);

        // ワーカータスクを起動
        let mut handles = Vec::new();
        for worker_id in 0..self.concurrency {
            let url = self.url.clone();
            let method = self.method.clone();
            let headers = self.headers.clone();
            let body = self.body.clone();
            let running = running.clone();
            let total = total.clone();
            let success = success.clone();
            let failed = failed.clone();
            let bytes_sent = bytes_sent.clone();
            let bytes_received = bytes_received.clone();
            let latencies = latencies.clone();
            let status_codes = status_codes.clone();
            let rate = self.rate;
            let concurrency = self.concurrency;

            let client = self.build_client()?;

            let handle = tokio::spawn(async move {
                let delay =
                    rate.map(|r| Duration::from_secs_f64(1.0 / r as f64 * concurrency as f64));

                while running.load(Ordering::Relaxed) {
                    let start = Instant::now();
                    let result = send_request(&client, &url, &method, &headers, &body).await;

                    total.fetch_add(1, Ordering::Relaxed);

                    match result {
                        Ok((status, sent, received)) => {
                            bytes_sent.fetch_add(sent, Ordering::Relaxed);
                            bytes_received.fetch_add(received, Ordering::Relaxed);
                            let latency = start.elapsed();
                            latencies.lock().await.add_duration(latency);

                            // ステータスコードをカウント
                            *status_codes.lock().await.entry(status).or_insert(0) += 1;

                            // 5xxはサーバエラーとして失敗扱い
                            if status >= 500 {
                                failed.fetch_add(1, Ordering::Relaxed);
                            } else {
                                success.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(e) => {
                            failed.fetch_add(1, Ordering::Relaxed);
                            debug!("Worker {} error: {}", worker_id, e);
                        }
                    }

                    // レート制限
                    if let Some(d) = delay {
                        let elapsed = start.elapsed();
                        if elapsed < d {
                            tokio::time::sleep(d - elapsed).await;
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // 時間経過を監視
        let duration = Duration::from_secs(self.duration_secs);
        let start = Instant::now();
        while start.elapsed() < duration {
            tokio::time::sleep(Duration::from_secs(1)).await;
            pb.inc(1);
        }

        // 停止シグナル
        running.store(false, Ordering::Relaxed);
        pb.finish_and_clear();

        // ワーカー終了を待機
        for handle in handles {
            let _ = handle.await;
        }

        // 結果を集計
        let elapsed = timer.elapsed_secs();
        let total_count = total.load(Ordering::Relaxed);
        let success_count = success.load(Ordering::Relaxed);
        let failed_count = failed.load(Ordering::Relaxed);
        let sent = bytes_sent.load(Ordering::Relaxed);
        let received = bytes_received.load(Ordering::Relaxed);

        let mut lat = latencies.lock().await;
        let latency_stats = lat.compute();

        // ステータスコード統計をログ出力
        let codes = status_codes.lock().await;
        debug!("Status codes: {:?}", *codes);

        Ok(LoadTestResult {
            target: self.url.clone(),
            protocol: "http".to_string(),
            duration_secs: elapsed,
            total_requests: total_count,
            successful_requests: success_count,
            failed_requests: failed_count,
            throughput_rps: if elapsed > 0.0 {
                total_count as f64 / elapsed
            } else {
                0.0
            },
            bytes_sent: sent,
            bytes_received: received,
            latency: latency_stats,
        })
    }
}

/// HTTPリクエストを送信
async fn send_request(
    client: &Client,
    url: &str,
    method: &Method,
    headers: &HashMap<String, String>,
    body: &Option<String>,
) -> Result<(u16, u64, u64)> {
    let mut request = client.request(method.clone(), url);

    // ヘッダーを追加
    for (key, value) in headers {
        request = request.header(key, value);
    }

    // ボディを追加
    let body_len = if let Some(b) = body {
        let len = b.len() as u64;
        request = request.body(b.clone());
        len
    } else {
        0
    };

    let response: Response = request
        .send()
        .await
        .map_err(|e| NelstError::connection_with_source("HTTP request failed".to_string(), e))?;

    let status = response.status().as_u16();
    let body_bytes = response.bytes().await.map_err(|e| {
        NelstError::connection_with_source("Failed to read response body".to_string(), e)
    })?;

    Ok((status, body_len, body_bytes.len() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::load::HttpArgs;

    fn create_test_args(
        url: &str,
        method: &str,
        headers: Vec<String>,
        body: Option<String>,
    ) -> HttpArgs {
        HttpArgs {
            url: url.to_string(),
            method: method.to_string(),
            headers,
            body,
            duration: 10,
            concurrency: 1,
            rate: None,
            insecure: false,
            follow_redirects: false,
            timeout: 5000,
            http2: false,
            output: None,
        }
    }

    #[test]
    fn test_http_test_new_valid_get() {
        let args = create_test_args("http://localhost:8080", "GET", vec![], None);
        let test = HttpTest::new(&args).unwrap();
        assert_eq!(test.url, "http://localhost:8080");
        assert_eq!(test.method, Method::GET);
        assert!(test.headers.is_empty());
        assert!(test.body.is_none());
    }

    #[test]
    fn test_http_test_new_valid_post_with_body() {
        let args = create_test_args(
            "http://localhost:8080/api",
            "POST",
            vec!["Content-Type: application/json".to_string()],
            Some(r#"{"key":"value"}"#.to_string()),
        );
        let test = HttpTest::new(&args).unwrap();
        assert_eq!(test.method, Method::POST);
        assert_eq!(
            test.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(test.body, Some(r#"{"key":"value"}"#.to_string()));
    }

    #[test]
    fn test_http_test_new_all_methods() {
        let methods = vec![
            ("GET", Method::GET),
            ("POST", Method::POST),
            ("PUT", Method::PUT),
            ("DELETE", Method::DELETE),
            ("PATCH", Method::PATCH),
            ("HEAD", Method::HEAD),
            ("OPTIONS", Method::OPTIONS),
        ];

        for (method_str, expected) in methods {
            let args = create_test_args("http://localhost:8080", method_str, vec![], None);
            let test = HttpTest::new(&args).unwrap();
            assert_eq!(
                test.method, expected,
                "Method {} should parse correctly",
                method_str
            );
        }
    }

    #[test]
    fn test_http_test_new_case_insensitive_method() {
        let cases = vec!["get", "Get", "GET", "gEt"];
        for method_str in cases {
            let args = create_test_args("http://localhost:8080", method_str, vec![], None);
            let test = HttpTest::new(&args).unwrap();
            assert_eq!(test.method, Method::GET);
        }
    }

    #[test]
    fn test_http_test_new_invalid_method() {
        let args = create_test_args("http://localhost:8080", "INVALID", vec![], None);
        let result = HttpTest::new(&args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Unknown HTTP method"));
    }

    #[test]
    fn test_http_test_new_valid_headers() {
        let args = create_test_args(
            "http://localhost:8080",
            "GET",
            vec![
                "Content-Type: application/json".to_string(),
                "Authorization: Bearer token123".to_string(),
                "X-Custom-Header: custom value with spaces".to_string(),
            ],
            None,
        );
        let test = HttpTest::new(&args).unwrap();
        assert_eq!(test.headers.len(), 3);
        assert_eq!(
            test.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            test.headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
        assert_eq!(
            test.headers.get("X-Custom-Header"),
            Some(&"custom value with spaces".to_string())
        );
    }

    #[test]
    fn test_http_test_new_header_whitespace_trimming() {
        let args = create_test_args(
            "http://localhost:8080",
            "GET",
            vec!["  Content-Type  :  application/json  ".to_string()],
            None,
        );
        let test = HttpTest::new(&args).unwrap();
        assert_eq!(
            test.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_http_test_new_invalid_header_format() {
        let args = create_test_args(
            "http://localhost:8080",
            "GET",
            vec!["InvalidHeaderWithoutColon".to_string()],
            None,
        );
        let result = HttpTest::new(&args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid header format"));
    }

    #[test]
    fn test_http_test_new_body_from_file_nonexistent() {
        let args = create_test_args(
            "http://localhost:8080",
            "POST",
            vec![],
            Some("@nonexistent_file_12345.json".to_string()),
        );
        let result = HttpTest::new(&args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to read body file"));
    }

    #[test]
    fn test_http_test_new_http2_flag() {
        let mut args = create_test_args("http://localhost:8080", "GET", vec![], None);
        args.http2 = true;
        let test = HttpTest::new(&args).unwrap();
        assert!(test.http2);
    }

    #[test]
    fn test_http_test_new_insecure_flag() {
        let mut args = create_test_args("https://localhost:8080", "GET", vec![], None);
        args.insecure = true;
        let test = HttpTest::new(&args).unwrap();
        assert!(test.insecure);
    }

    #[test]
    fn test_http_test_new_follow_redirects_flag() {
        let mut args = create_test_args("http://localhost:8080", "GET", vec![], None);
        args.follow_redirects = true;
        let test = HttpTest::new(&args).unwrap();
        assert!(test.follow_redirects);
    }

    #[test]
    fn test_http_test_new_timeout_conversion() {
        let mut args = create_test_args("http://localhost:8080", "GET", vec![], None);
        args.timeout = 10000; // 10秒（ミリ秒）
        let test = HttpTest::new(&args).unwrap();
        assert_eq!(test.timeout, Duration::from_millis(10000));
    }

    #[test]
    fn test_http_test_build_client_success() {
        let args = create_test_args("http://localhost:8080", "GET", vec![], None);
        let test = HttpTest::new(&args).unwrap();
        let client = test.build_client();
        assert!(client.is_ok());
    }
}
