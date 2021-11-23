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


# 参照

[tokio-rs/mio - GitHub](https://github.com/tokio-rs/mio)
