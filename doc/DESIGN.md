# 設計

設計メモに近いかもしれない。
随時整理していく。

低レイヤの処理も作る想定だったので、
[tokio-rs/mio - GitHub](https://github.com/tokio-rs/mio)
を使う想定だったが、まだRust自体の理解が追いつかないので
[tokio-rs/tokio - GitHub](https://github.com/tokio-rs/tokio)
で作成していく。

# 負荷テスト

## データ受信
### Client仕様
ターゲットへ指定したデータサイズのパケットを送信し続ける。

### Server仕様
クライアントから受信し、エコーまたは指定したデータサイズのパケットを返す。
UDPも作成するつもりだが、便宜上Serverと呼ぶ。

## コネクション

# 参照

[tokio-rs/mio - GitHub](https://github.com/tokio-rs/mio)
[tokio-rs/tokio - GitHub](https://github.com/tokio-rs/tokio)
