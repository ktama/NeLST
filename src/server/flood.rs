//! フラッドサーバモジュール
//!
//! 指定サイズのデータを継続送信するテストサーバ。

use crate::cli::server::FloodServerArgs;
use crate::common::error::Result;

/// フラッドサーバを起動
pub async fn run(_args: &FloodServerArgs) -> Result<()> {
    // TODO: フェーズ1で実装
    todo!("Flood server not implemented yet")
}
