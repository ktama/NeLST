//! レポート生成モジュール
//!
//! テスト結果を各種フォーマットで出力する機能を提供する。

mod formatter;

pub use formatter::{ReportFormat, ReportGenerator};
