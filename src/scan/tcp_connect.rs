//! TCP Connectスキャンモジュール
//!
//! 通常のTCP 3ウェイハンドシェイクを完了させてポートの開閉を判定する。

use crate::cli::scan::PortScanArgs;
use crate::common::error::Result;
use serde::{Deserialize, Serialize};

/// ポートの状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortState {
    Open,
    Closed,
    Filtered,
}

impl std::fmt::Display for PortState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortState::Open => write!(f, "open"),
            PortState::Closed => write!(f, "closed"),
            PortState::Filtered => write!(f, "filtered"),
        }
    }
}

/// スキャン結果（1ポート分）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortResult {
    pub port: u16,
    pub state: PortState,
    pub service: Option<String>,
}

/// スキャン結果（全体）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub target: String,
    pub method: String,
    pub scan_time: String,
    pub duration_secs: f64,
    pub ports: Vec<PortResult>,
    pub summary: ScanSummary,
}

/// スキャンサマリー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub total_scanned: usize,
    pub open: usize,
    pub closed: usize,
    pub filtered: usize,
}

impl std::fmt::Display for ScanResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  PORT      STATE    SERVICE")?;
        for port in &self.ports {
            if port.state == PortState::Open {
                writeln!(
                    f,
                    "  {}/tcp    {:8} {}",
                    port.port,
                    port.state,
                    port.service.as_deref().unwrap_or("-")
                )?;
            }
        }
        writeln!(f)?;
        writeln!(f, "Scan completed in {:.2}s", self.duration_secs)?;
        writeln!(
            f,
            "Open: {}, Closed: {}, Filtered: {}",
            self.summary.open, self.summary.closed, self.summary.filtered
        )?;
        Ok(())
    }
}

/// TCP Connectスキャンを実行
pub async fn run(_args: &PortScanArgs) -> Result<ScanResult> {
    // TODO: フェーズ1で実装
    todo!("TCP Connect scan not implemented yet")
}
