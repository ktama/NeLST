//! 設定管理モジュール
//!
//! 設定ファイルの読み込みとCLI引数との統合を行う。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::common::error::{NelstError, Result};

/// アプリケーション設定
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// デフォルト設定
    #[serde(default)]
    pub defaults: DefaultsConfig,

    /// 負荷テスト設定
    #[serde(default)]
    pub load: LoadConfig,

    /// スキャン設定
    #[serde(default)]
    pub scan: ScanConfig,

    /// サーバ設定
    #[serde(default)]
    pub server: ServerConfig,
}

/// デフォルト設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    /// 詳細ログ出力
    #[serde(default)]
    pub verbose: bool,

    /// タイムアウト（ミリ秒）
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            timeout: default_timeout(),
        }
    }
}

fn default_timeout() -> u64 {
    5000
}

/// 負荷テスト設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfig {
    /// プロトコル
    #[serde(default = "default_protocol")]
    pub protocol: String,

    /// 同時接続数
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,

    /// テスト継続時間（秒）
    #[serde(default = "default_duration")]
    pub duration: u64,

    /// パケットサイズ（バイト）
    #[serde(default = "default_size")]
    pub size: usize,
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            protocol: default_protocol(),
            concurrency: default_concurrency(),
            duration: default_duration(),
            size: default_size(),
        }
    }
}

fn default_protocol() -> String {
    "tcp".to_string()
}

fn default_concurrency() -> usize {
    10
}

fn default_duration() -> u64 {
    60
}

fn default_size() -> usize {
    1024
}

/// スキャン設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// スキャン手法
    #[serde(default = "default_method")]
    pub method: String,

    /// ポート範囲
    #[serde(default = "default_ports")]
    pub ports: String,

    /// 並列スキャン数
    #[serde(default = "default_scan_concurrency")]
    pub concurrency: usize,

    /// タイムアウト（ミリ秒）
    #[serde(default = "default_scan_timeout")]
    pub timeout: u64,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            method: default_method(),
            ports: default_ports(),
            concurrency: default_scan_concurrency(),
            timeout: default_scan_timeout(),
        }
    }
}

fn default_method() -> String {
    "tcp".to_string()
}

fn default_ports() -> String {
    "1-1024".to_string()
}

fn default_scan_concurrency() -> usize {
    100
}

fn default_scan_timeout() -> u64 {
    1000
}

/// サーバ設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// バインドアドレス
    #[serde(default = "default_bind")]
    pub bind: String,

    /// プロトコル
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            protocol: default_protocol(),
        }
    }
}

fn default_bind() -> String {
    "0.0.0.0:8080".to_string()
}

impl Config {
    /// 設定ファイルを読み込む
    ///
    /// 優先順位:
    /// 1. 指定されたパス
    /// 2. ./nelst.toml
    /// 3. ~/.nelst/config.toml
    pub fn load(path: Option<&str>) -> Result<Self> {
        // 指定されたパスがあれば最優先
        if let Some(p) = path {
            return Self::load_from_path(PathBuf::from(p));
        }

        // ./nelst.toml を試行
        let local_config = PathBuf::from("nelst.toml");
        if local_config.exists() {
            return Self::load_from_path(local_config);
        }

        // ~/.nelst/config.toml を試行
        if let Some(home) = dirs::home_dir() {
            let home_config = home.join(".nelst").join("config.toml");
            if home_config.exists() {
                return Self::load_from_path(home_config);
            }
        }

        // 設定ファイルがなければデフォルト設定を返す
        Ok(Self::default())
    }

    /// 指定されたパスから設定ファイルを読み込む
    fn load_from_path(path: PathBuf) -> Result<Self> {
        let content = fs::read_to_string(&path).map_err(|e| {
            NelstError::config(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        toml::from_str(&content)
            .map_err(|e| NelstError::config(format!("Failed to parse config file: {}", e)))
    }

    /// 設定ディレクトリのパスを取得
    pub fn config_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".nelst"))
    }

    /// プロファイルディレクトリのパスを取得
    pub fn profiles_dir() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("profiles"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.defaults.timeout, 5000);
        assert_eq!(config.load.protocol, "tcp");
        assert_eq!(config.scan.ports, "1-1024");
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let config = Config::load(None).unwrap();
        assert_eq!(config.defaults.timeout, 5000);
    }
}
