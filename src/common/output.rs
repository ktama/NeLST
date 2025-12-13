//! 出力ユーティリティモジュール
//!
//! テキスト出力、JSON出力、プログレスバー表示などを提供する。

#![allow(dead_code)]

use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use std::io::{self, Write};
use std::time::Duration;

/// 出力フォーマット
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Quiet,
}

/// 出力ハンドラ
pub struct Output {
    format: OutputFormat,
}

impl Output {
    /// 新しい出力ハンドラを作成
    pub fn new(json: bool, quiet: bool) -> Self {
        let format = if quiet {
            OutputFormat::Quiet
        } else if json {
            OutputFormat::Json
        } else {
            OutputFormat::Text
        };
        Self { format }
    }

    /// フォーマットを取得
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// JSON出力モードかどうか
    pub fn is_json(&self) -> bool {
        self.format == OutputFormat::Json
    }

    /// ヘッダーを出力（テキストモードのみ）
    pub fn header(&self, title: &str) {
        if self.format != OutputFormat::Text {
            return;
        }
        println!();
        println!("NeLST - {}", title);
        println!("{}", "━".repeat(57));
        println!();
    }

    /// セクションヘッダーを出力（テキストモードのみ）
    pub fn section(&self, title: &str) {
        if self.format != OutputFormat::Text {
            return;
        }
        let padding = (45 - title.len()) / 2;
        let separator = "━".repeat(padding.max(3));
        println!("{} {} {}", separator, title, separator);
        println!();
    }

    /// 情報を出力（テキストモードのみ）
    pub fn info(&self, label: &str, value: &str) {
        if self.format != OutputFormat::Text {
            return;
        }
        println!("  {:<15} {}", format!("{}:", label), value);
    }

    /// メッセージを出力（テキストモードのみ）
    pub fn message(&self, msg: &str) {
        if self.format != OutputFormat::Text {
            return;
        }
        println!("{}", msg);
    }

    /// 結果をJSON形式で出力
    pub fn json<T: Serialize>(&self, value: &T) -> io::Result<()> {
        if self.format != OutputFormat::Json {
            return Ok(());
        }
        let json = serde_json::to_string_pretty(value)?;
        println!("{}", json);
        Ok(())
    }

    /// 結果を出力（形式に応じて自動選択）
    pub fn result<T: Serialize + std::fmt::Display>(&self, value: &T) -> io::Result<()> {
        match self.format {
            OutputFormat::Text => {
                println!("{}", value);
                Ok(())
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(value)?;
                println!("{}", json);
                Ok(())
            }
            OutputFormat::Quiet => Ok(()),
        }
    }

    /// エラーを出力
    pub fn error(&self, msg: &str) {
        eprintln!("Error: {}", msg);
    }

    /// 成功メッセージを出力（テキストモードのみ）
    pub fn success(&self, msg: &str) {
        if self.format != OutputFormat::Text {
            return;
        }
        println!("✓ {}", msg);
    }

    /// 警告メッセージを出力（テキストモードのみ）
    pub fn warning(&self, msg: &str) {
        if self.format != OutputFormat::Text {
            return;
        }
        println!("⚠ {}", msg);
    }

    /// エラーとヒントを出力
    pub fn error_with_hint(&self, msg: &str, hint: &str) {
        eprintln!("Error: {}", msg);
        eprintln!("Hint: {}", hint);
    }

    /// 改行を出力
    pub fn newline(&self) {
        if self.format == OutputFormat::Text {
            println!();
        }
    }

    /// フラッシュ
    pub fn flush(&self) -> io::Result<()> {
        io::stdout().flush()
    }
}

/// プログレスバーを作成
pub fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb.set_message(message.to_string());
    pb
}

/// スピナーを作成
pub fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// 時間経過表示付きプログレスバーを作成
pub fn create_duration_progress_bar(duration_secs: u64) -> ProgressBar {
    let pb = ProgressBar::new(duration_secs);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("Running... [{bar:40.green/black}] {pos}s/{len}s")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format() {
        let output = Output::new(false, false);
        assert_eq!(output.format(), OutputFormat::Text);

        let output = Output::new(true, false);
        assert_eq!(output.format(), OutputFormat::Json);

        let output = Output::new(false, true);
        assert_eq!(output.format(), OutputFormat::Quiet);

        // quiet が優先
        let output = Output::new(true, true);
        assert_eq!(output.format(), OutputFormat::Quiet);
    }
}
