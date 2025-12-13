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
    use std::env;

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

    #[test]
    fn test_config_dir() {
        let config_dir = Config::config_dir();
        assert!(config_dir.is_some());
        let path = config_dir.unwrap();
        assert!(path.to_string_lossy().contains(".nelst"));
    }

    #[test]
    fn test_profiles_dir() {
        let profiles_dir = Config::profiles_dir();
        assert!(profiles_dir.is_some());
        let path = profiles_dir.unwrap();
        assert!(path.to_string_lossy().contains("profiles"));
    }

    #[test]
    fn test_load_from_explicit_path() {
        // 存在しないパスを指定
        let result = Config::load(Some("/nonexistent/path/config.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_from_valid_toml() {
        use std::io::Write;

        let temp_dir = env::temp_dir().join(format!("nelst_config_test_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let config_path = temp_dir.join("test_config.toml");

        let config_content = r#"
[defaults]
verbose = true
timeout = 10000

[load]
protocol = "udp"
concurrency = 20
duration = 120
size = 2048

[scan]
method = "syn"
ports = "1-65535"
concurrency = 200
timeout = 2000

[server]
bind = "127.0.0.1:9090"
protocol = "udp"
"#;

        let mut file = fs::File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::load(Some(config_path.to_str().unwrap())).unwrap();

        assert!(config.defaults.verbose);
        assert_eq!(config.defaults.timeout, 10000);
        assert_eq!(config.load.protocol, "udp");
        assert_eq!(config.load.concurrency, 20);
        assert_eq!(config.scan.ports, "1-65535");
        assert_eq!(config.server.bind, "127.0.0.1:9090");

        // クリーンアップ
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_config_partial_toml() {
        use std::io::Write;

        let temp_dir = env::temp_dir().join(format!("nelst_config_partial_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let config_path = temp_dir.join("partial_config.toml");

        // 一部のセクションのみ指定
        let config_content = r#"
[defaults]
timeout = 3000
"#;

        let mut file = fs::File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::load(Some(config_path.to_str().unwrap())).unwrap();

        // 指定した値
        assert_eq!(config.defaults.timeout, 3000);
        // デフォルト値が使用される
        assert!(!config.defaults.verbose);
        assert_eq!(config.load.protocol, "tcp");
        assert_eq!(config.scan.ports, "1-1024");

        // クリーンアップ
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_config_invalid_toml() {
        use std::io::Write;

        let temp_dir = env::temp_dir().join(format!("nelst_config_invalid_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let config_path = temp_dir.join("invalid_config.toml");

        // 不正なTOML
        let config_content = r#"
[defaults
timeout = 5000
"#;

        let mut file = fs::File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let result = Config::load(Some(config_path.to_str().unwrap()));
        assert!(result.is_err());

        // クリーンアップ
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_defaults_config_default() {
        let defaults = DefaultsConfig::default();
        assert!(!defaults.verbose);
        assert_eq!(defaults.timeout, 5000);
    }

    #[test]
    fn test_load_config_default() {
        let load = LoadConfig::default();
        assert_eq!(load.protocol, "tcp");
        assert_eq!(load.concurrency, 10);
        assert_eq!(load.duration, 60);
        assert_eq!(load.size, 1024);
    }

    #[test]
    fn test_scan_config_default() {
        let scan = ScanConfig::default();
        assert_eq!(scan.method, "tcp");
        assert_eq!(scan.ports, "1-1024");
        assert_eq!(scan.concurrency, 100);
        assert_eq!(scan.timeout, 1000);
    }

    #[test]
    fn test_server_config_default() {
        let server = ServerConfig::default();
        assert_eq!(server.bind, "0.0.0.0:8080");
        assert_eq!(server.protocol, "tcp");
    }
}
