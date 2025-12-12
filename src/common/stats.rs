//! 統計収集モジュール
//!
//! テスト結果の統計情報（レイテンシ、スループット等）を収集・計算する。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// レイテンシ統計
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    /// 最小レイテンシ（マイクロ秒）
    pub min_us: u64,
    /// 最大レイテンシ（マイクロ秒）
    pub max_us: u64,
    /// 平均レイテンシ（マイクロ秒）
    pub avg_us: u64,
    /// P50（中央値）レイテンシ（マイクロ秒）
    pub p50_us: u64,
    /// P95レイテンシ（マイクロ秒）
    pub p95_us: u64,
    /// P99レイテンシ（マイクロ秒）
    pub p99_us: u64,
}

impl LatencyStats {
    /// ミリ秒で取得
    pub fn min_ms(&self) -> f64 {
        self.min_us as f64 / 1000.0
    }

    pub fn max_ms(&self) -> f64 {
        self.max_us as f64 / 1000.0
    }

    pub fn avg_ms(&self) -> f64 {
        self.avg_us as f64 / 1000.0
    }

    pub fn p50_ms(&self) -> f64 {
        self.p50_us as f64 / 1000.0
    }

    pub fn p95_ms(&self) -> f64 {
        self.p95_us as f64 / 1000.0
    }

    pub fn p99_ms(&self) -> f64 {
        self.p99_us as f64 / 1000.0
    }
}

/// 負荷テスト結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestResult {
    /// ターゲット
    pub target: String,
    /// プロトコル
    pub protocol: String,
    /// テスト継続時間（秒）
    pub duration_secs: f64,
    /// 総リクエスト数
    pub total_requests: u64,
    /// 成功したリクエスト数
    pub successful_requests: u64,
    /// 失敗したリクエスト数
    pub failed_requests: u64,
    /// スループット（リクエスト/秒）
    pub throughput_rps: f64,
    /// 送信バイト数
    pub bytes_sent: u64,
    /// 受信バイト数
    pub bytes_received: u64,
    /// レイテンシ統計
    pub latency: Option<LatencyStats>,
}

impl LoadTestResult {
    /// 成功率を計算
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.successful_requests as f64 / self.total_requests as f64) * 100.0
    }
}

impl std::fmt::Display for LoadTestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  Total Requests:     {:>10}", self.total_requests)?;
        writeln!(
            f,
            "  Successful:         {:>10} ({:.2}%)",
            self.successful_requests,
            self.success_rate()
        )?;
        writeln!(
            f,
            "  Failed:             {:>10} ({:.2}%)",
            self.failed_requests,
            100.0 - self.success_rate()
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "  Throughput:         {:>10.2} req/s",
            self.throughput_rps
        )?;
        writeln!(
            f,
            "  Data Transferred:   {:>10}",
            format_bytes(self.bytes_sent + self.bytes_received)
        )?;

        if let Some(ref latency) = self.latency {
            writeln!(f)?;
            writeln!(f, "  Latency:")?;
            writeln!(f, "    Min:    {:>8.2} ms", latency.min_ms())?;
            writeln!(f, "    Max:    {:>8.2} ms", latency.max_ms())?;
            writeln!(f, "    Avg:    {:>8.2} ms", latency.avg_ms())?;
            writeln!(f, "    P50:    {:>8.2} ms", latency.p50_ms())?;
            writeln!(f, "    P95:    {:>8.2} ms", latency.p95_ms())?;
            writeln!(f, "    P99:    {:>8.2} ms", latency.p99_ms())?;
        }

        Ok(())
    }
}

/// レイテンシコレクター
#[derive(Debug)]
pub struct LatencyCollector {
    samples: Vec<u64>,
}

impl LatencyCollector {
    /// 新しいコレクターを作成
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(10000),
        }
    }

    /// 容量を指定して作成
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
        }
    }

    /// サンプルを追加（マイクロ秒）
    pub fn add(&mut self, latency_us: u64) {
        self.samples.push(latency_us);
    }

    /// Duration からサンプルを追加
    pub fn add_duration(&mut self, duration: Duration) {
        self.samples.push(duration.as_micros() as u64);
    }

    /// サンプル数を取得
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// 統計を計算
    pub fn compute(&mut self) -> Option<LatencyStats> {
        if self.samples.is_empty() {
            return None;
        }

        self.samples.sort_unstable();

        let len = self.samples.len();
        let sum: u64 = self.samples.iter().sum();

        Some(LatencyStats {
            min_us: self.samples[0],
            max_us: self.samples[len - 1],
            avg_us: sum / len as u64,
            p50_us: self.percentile(50),
            p95_us: self.percentile(95),
            p99_us: self.percentile(99),
        })
    }

    fn percentile(&self, p: usize) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        // パーセンタイルインデックスを計算（小数点以下切り捨て）
        let idx = ((self.samples.len() as f64 * p as f64 / 100.0).ceil() as usize)
            .saturating_sub(1)
            .min(self.samples.len() - 1);
        self.samples[idx]
    }
}

impl Default for LatencyCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// カウンター
#[derive(Debug, Default)]
pub struct Counter {
    pub total: u64,
    pub success: u64,
    pub failed: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl Counter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&mut self, sent: u64, received: u64) {
        self.total += 1;
        self.success += 1;
        self.bytes_sent += sent;
        self.bytes_received += received;
    }

    pub fn record_failure(&mut self) {
        self.total += 1;
        self.failed += 1;
    }
}

/// タイマー
#[derive(Debug)]
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

/// バイト数をフォーマット
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_collector() {
        let mut collector = LatencyCollector::new();
        for i in 1..=100 {
            collector.add(i * 1000); // 1ms to 100ms
        }

        let stats = collector.compute().unwrap();
        assert_eq!(stats.min_us, 1000);
        assert_eq!(stats.max_us, 100000);
        assert_eq!(stats.p50_us, 50000);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_counter() {
        let mut counter = Counter::new();
        counter.record_success(100, 100);
        counter.record_success(100, 100);
        counter.record_failure();

        assert_eq!(counter.total, 3);
        assert_eq!(counter.success, 2);
        assert_eq!(counter.failed, 1);
        assert_eq!(counter.bytes_sent, 200);
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(timer.elapsed_secs() >= 0.01);
    }

    #[test]
    fn test_load_test_result_success_rate() {
        let result = LoadTestResult {
            target: "localhost".to_string(),
            protocol: "tcp".to_string(),
            duration_secs: 1.0,
            total_requests: 100,
            successful_requests: 80,
            failed_requests: 20,
            throughput_rps: 100.0,
            bytes_sent: 1000,
            bytes_received: 1000,
            latency: None,
        };
        assert!((result.success_rate() - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_latency_collector_empty() {
        let mut collector = LatencyCollector::new();
        assert!(collector.compute().is_none());
    }

    #[test]
    fn test_latency_stats_ms_conversion() {
        let stats = LatencyStats {
            min_us: 1000,
            max_us: 10000,
            avg_us: 5000,
            p50_us: 5000,
            p95_us: 9500,
            p99_us: 9900,
        };
        assert!((stats.min_ms() - 1.0).abs() < 0.01);
        assert!((stats.max_ms() - 10.0).abs() < 0.01);
    }
}
