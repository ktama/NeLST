//! エラーハンドリングモジュール
//!
//! カスタムエラー型と終了コードを定義する。

#![allow(dead_code)]

use std::process::ExitCode;
use thiserror::Error;

/// NeLST の終了コード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitStatus {
    /// 正常終了
    Success = 0,
    /// 一般的なエラー
    GeneralError = 1,
    /// 引数エラー
    ArgumentError = 2,
    /// 接続エラー
    ConnectionError = 3,
    /// 権限エラー（要root）
    PermissionError = 4,
    /// タイムアウト
    TimeoutError = 5,
}

impl From<ExitStatus> for ExitCode {
    fn from(status: ExitStatus) -> Self {
        ExitCode::from(status as u8)
    }
}

/// NeLST のエラー型
#[derive(Error, Debug)]
pub enum NelstError {
    /// 引数エラー
    #[error("Argument error: {message}")]
    Argument {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 接続エラー
    #[error("Connection error: {message}")]
    Connection {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 権限エラー
    #[error("Permission denied: {message}")]
    Permission {
        message: String,
        hint: Option<String>,
    },

    /// タイムアウトエラー
    #[error("Timeout: {message}")]
    Timeout { message: String },

    /// I/Oエラー
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// 設定エラー
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// スキャンエラー
    #[error("Scan error: {message}")]
    Scan { message: String },

    /// その他のエラー
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl NelstError {
    /// エラーに対応する終了コードを返す
    pub fn exit_status(&self) -> ExitStatus {
        match self {
            NelstError::Argument { .. } => ExitStatus::ArgumentError,
            NelstError::Connection { .. } => ExitStatus::ConnectionError,
            NelstError::Permission { .. } => ExitStatus::PermissionError,
            NelstError::Timeout { .. } => ExitStatus::TimeoutError,
            NelstError::Io(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    ExitStatus::PermissionError
                } else if e.kind() == std::io::ErrorKind::TimedOut {
                    ExitStatus::TimeoutError
                } else {
                    ExitStatus::GeneralError
                }
            }
            _ => ExitStatus::GeneralError,
        }
    }

    /// ヒントメッセージがあれば返す
    pub fn hint(&self) -> Option<&str> {
        match self {
            NelstError::Permission { hint, .. } => hint.as_deref(),
            _ => None,
        }
    }

    /// 接続エラーを生成するヘルパー
    pub fn connection(message: impl Into<String>) -> Self {
        NelstError::Connection {
            message: message.into(),
            source: None,
        }
    }

    /// 接続エラーを生成するヘルパー（ソース付き）
    pub fn connection_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        NelstError::Connection {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// 権限エラーを生成するヘルパー
    pub fn permission(message: impl Into<String>) -> Self {
        NelstError::Permission {
            message: message.into(),
            hint: None,
        }
    }

    /// 権限エラーを生成するヘルパー（ヒント付き）
    pub fn permission_with_hint(message: impl Into<String>, hint: impl Into<String>) -> Self {
        NelstError::Permission {
            message: message.into(),
            hint: Some(hint.into()),
        }
    }

    /// タイムアウトエラーを生成するヘルパー
    pub fn timeout(message: impl Into<String>) -> Self {
        NelstError::Timeout {
            message: message.into(),
        }
    }

    /// 引数エラーを生成するヘルパー
    pub fn argument(message: impl Into<String>) -> Self {
        NelstError::Argument {
            message: message.into(),
            source: None,
        }
    }

    /// 設定エラーを生成するヘルパー
    pub fn config(message: impl Into<String>) -> Self {
        NelstError::Config {
            message: message.into(),
        }
    }

    /// スキャンエラーを生成するヘルパー
    pub fn scan(message: impl Into<String>) -> Self {
        NelstError::Scan {
            message: message.into(),
        }
    }

    /// I/Oエラーからの変換（メッセージ付き）
    pub fn io_with_context(message: impl Into<String>, source: std::io::Error) -> Self {
        NelstError::Connection {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// エラーが再試行可能かどうかを判定
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            NelstError::Connection { .. } | NelstError::Timeout { .. }
        )
    }
}

/// NeLST の結果型
pub type Result<T> = std::result::Result<T, NelstError>;

/// エラーメッセージをフォーマットして表示
pub fn format_error(error: &NelstError) -> String {
    let mut output = format!("Error: {}", error);

    if let Some(hint) = error.hint() {
        output.push_str(&format!("\nHint: {}", hint));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_status_conversion() {
        assert_eq!(ExitCode::from(ExitStatus::Success), ExitCode::from(0));
        assert_eq!(ExitCode::from(ExitStatus::GeneralError), ExitCode::from(1));
        assert_eq!(
            ExitCode::from(ExitStatus::PermissionError),
            ExitCode::from(4)
        );
    }

    #[test]
    fn test_error_with_hint() {
        let err = NelstError::permission_with_hint(
            "SYN scan requires root privileges",
            "Run with 'sudo nelst scan port -m syn ...'",
        );
        assert!(err.hint().is_some());
        assert!(err.hint().unwrap().contains("sudo"));
    }

    #[test]
    fn test_error_exit_status_mapping() {
        assert_eq!(
            NelstError::argument("bad arg").exit_status(),
            ExitStatus::ArgumentError
        );
        assert_eq!(
            NelstError::connection("failed").exit_status(),
            ExitStatus::ConnectionError
        );
        assert_eq!(
            NelstError::timeout("timed out").exit_status(),
            ExitStatus::TimeoutError
        );
        assert_eq!(
            NelstError::permission("denied").exit_status(),
            ExitStatus::PermissionError
        );
    }

    #[test]
    fn test_is_retryable() {
        assert!(NelstError::connection("failed").is_retryable());
        assert!(NelstError::timeout("timed out").is_retryable());
        assert!(!NelstError::argument("bad arg").is_retryable());
        assert!(!NelstError::permission("denied").is_retryable());
    }

    #[test]
    fn test_format_error() {
        let err = NelstError::permission_with_hint("Root required", "Use sudo");
        let formatted = format_error(&err);
        assert!(formatted.contains("Error:"));
        assert!(formatted.contains("Hint:"));
    }
}
