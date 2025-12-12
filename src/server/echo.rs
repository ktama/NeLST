//! エコーサーバモジュール
//!
//! 受信したデータをそのまま返すサーバ。

use crate::cli::server::EchoServerArgs;
use crate::common::error::Result;

/// エコーサーバを起動
pub async fn run(_args: &EchoServerArgs) -> Result<()> {
    // TODO: フェーズ1で実装
    todo!("Echo server not implemented yet")
}
