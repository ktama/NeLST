//! プロファイル管理モジュール
//!
//! よく使う設定をプロファイルとして保存・読み込みする機能を提供する。

mod manager;

pub use manager::{Profile, ProfileManager};
