# 設計

設計メモに近いかもしれない。
随時整理していく。

[tokio-rs/mio - GitHub](https://github.com/tokio-rs/mio)

# 負荷テスト

## データ受信
### Client仕様
ターゲットへ指定したデータサイズのパケットを送信し続ける。


### Server仕様
クライアントから受信し、エコーまたは指定したデータサイズのパケットを返す。
UDPも作成するつもりだが、便宜上Serverと呼ぶ。

## コネクション
TCPのみ

### Client仕様
コネクションを繰り返し、大量のコネクションを確立

### Server仕様
負荷テストにならないかもしれないが、コネクションを受ける

# ポートスキャンテスト

* TCPスキャン
* SYNスキャン
* FINスキャン
* クリスマスツリースキャン
* NULLスキャン
* UDPスキャン


# 参照

[tokio-rs/mio - GitHub](https://github.com/tokio-rs/mio)
[Crate mio - Rust](https://docs.rs/mio/0.8.0/mio/index.html)
[Struct std::sync::RwLock - Rust](https://doc.rust-lang.org/std/sync/struct.RwLock.html)
[Rust入門 Chapter 18 並列処理 - Zenn](https://zenn.dev/mebiusbox/books/22d4c1ed9b0003/viewer/98dc80)
[Rust はどのようにして安全な並列処理を提供するのか - Qiita](https://qiita.com/nirasan/items/97263103f076bd525a7b)