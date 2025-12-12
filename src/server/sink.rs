//! シンクサーバモジュール
//!
//! 受信したデータを破棄する（応答なし）サーバ。

use crate::cli::server::SinkServerArgs;
use crate::common::error::Result;

/// シンクサーバを起動
pub async fn run(_args: &SinkServerArgs) -> Result<()> {
    // TODO: フェーズ1で実装
    todo!("Sink server not implemented yet")
}
